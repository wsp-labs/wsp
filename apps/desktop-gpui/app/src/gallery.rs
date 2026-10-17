// SPDX-License-Identifier: AGPL-3.0-only
//! `desktop-gpui --gallery`: every shared piece in every state, in the current theme, with the theme picker at the top.
//! Every later screen is checked against it. Its words and pieces match a capture of the web app's own components.

use gpui_kit::component::input::InputState;
use gpui_kit::component::{h_flex, v_flex};
use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::{canvas, div, prelude::*, px, AnyElement, App, Bounds, Context, Entity, Pixels, Subscription, Window};

use crate::ui::dialog::shortcut_tooltip_view;
use crate::ui::grid::LIST_COLUMNS;
use crate::ui::{
    chevron, glyph, glyph_frame, num, quiet, section_head, size, slash, spinner, theme, ActivePalette, AlertDialog, Button, Card, CardItem,
    Chip, Choice, CopyRow, Crab, Dialog, Field, Glyph, Grid, GridHead, GridName, GridRow, HeadCell, HeadRow, KeyCaps, Line, PickRow,
    Refusal, Row, RowSkeleton, Segment, SegmentedControl, Select, SelectEvent, SelectItem, SelectState, Skeleton, StateCell, Status,
    StatusMark, Step, StepRow, Switch, VersionFact, LOCKUP,
};

/// The width every section's content takes: a Settings page's column, `max-w-[760px]` less its `px-8`
/// (`SettingsPage.tsx`).
pub const CONTENT: Pixels = px(696.);
/// The padding round a section.
pub const PAD: Pixels = px(24.);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Card,
    Rows,
    Controls,
    Dialog,
    Status,
    Lists,
}

impl Section {
    pub const ALL: [Section; 6] = [Section::Card, Section::Rows, Section::Controls, Section::Dialog, Section::Status, Section::Lists];

    pub fn name(self) -> &'static str {
        match self {
            Section::Card => "card",
            Section::Rows => "rows",
            Section::Controls => "controls",
            Section::Dialog => "dialog",
            Section::Status => "status",
            Section::Lists => "lists",
        }
    }

    /// The head the full gallery shows over the section.
    pub fn title(self) -> &'static str {
        match self {
            Section::Card => "The card and its head",
            Section::Rows => "Rows and lines",
            Section::Controls => "Controls, picks and buttons",
            Section::Dialog => "The dialog shell and the copy row",
            Section::Status => "State, loading and shortcuts",
            Section::Lists => "The head row, the list grid and loading",
        }
    }
}

/// What a capture draws alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Only {
    Section(Section),
    Piece(Piece),
}

macro_rules! pieces {
    ($($piece:ident $name:literal $wide:literal,)*) => {
        /// Each piece alone, for a capture beside the web component's.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Piece {
            $($piece,)*
        }

        impl Piece {
            pub const ALL: &[Piece] = &[$(Piece::$piece,)*];

            pub fn name(self) -> &'static str {
                match self {
                    $(Piece::$piece => $name,)*
                }
            }

            /// Whether the piece takes the column's width, as a card does, or its own.
            pub fn wide(self) -> bool {
                match self {
                    $(Piece::$piece => $wide,)*
                }
            }
        }
    };
}

pieces! {
    SectionHead "section-head" true,
    Card "card" true,
    CardLede "card-lede" true,
    RowSelect "row-select" true,
    RowField "row-field" true,
    RowButton "row-button" true,
    RowLead "row-lead" true,
    RowOpen "row-open" true,
    RowWarning "row-warning" true,
    LineValue "line-value" true,
    LineKeys "line-keys" true,
    LineRange "line-range" true,
    LineEmpty "line-empty" true,
    LineFact "line-fact" true,
    RowReset "row-reset" true,
    RowMark "row-mark" true,
    RowMono "row-mono" true,
    RowClip "row-clip" true,
    RowChips "row-chips" true,
    RowSpaced "row-spaced" true,
    RowOpenControl "row-open-control" true,
    HeadRow "head-row" true,
    Grid "grid" true,
    RowSkeleton "row-skeleton" true,
    Skeleton "skeleton" false,
    Crumbs "crumbs" false,
    Lockup "lockup" false,
    GlyphFrame "glyph-frame" false,
    Select "select" false,
    SelectHeld "select-held" false,
    SelectOpen "select-open" false,
    Field "field" false,
    FieldEmpty "field-empty" false,
    FieldFocused "field-focused" false,
    RowSwitch "row-switch" true,
    RowSegmented "row-segmented" true,
    Switch "switch" false,
    SwitchOff "switch-off" false,
    SwitchHeld "switch-held" false,
    Segmented "segmented" false,
    SegmentedLarge "segmented-large" false,
    Choice "choice" true,
    Pick "pick" true,
    ButtonXs "button-xs" false,
    ButtonXsHeld "button-xs-held" false,
    ButtonOutline "button-outline" false,
    ButtonPrimary "button-primary" false,
    ButtonPrimaryHeld "button-primary-held" false,
    ButtonText "button-text" false,
    ButtonGhost "button-ghost" false,
    ButtonAdd "button-add" false,
    ButtonIcon "button-icon" false,
    Keycaps "keycaps" false,
    Dialog "dialog" false,
    AlertDialog "alert-dialog" false,
    Sheet "sheet" false,
    Refusal "refusal" true,
    FieldHeld "field-held" false,
    FieldInvalid "field-invalid" false,
    ButtonDanger "button-danger" false,
    ButtonDestructive "button-destructive" false,
    CardLocked "card-locked" true,
    CardHeadLink "card-head-link" true,
    CopyRow "copy-row" false,
    Status "status" true,
    Steps "steps" true,
    Spinner "spinner" false,
    Crab "crab" false,
    Tooltip "tooltip" false,
}

pub struct Gallery {
    /// One section or piece alone, for a capture; every section under the theme picker otherwise.
    only: Option<Only>,
    theme: Entity<SelectState<&'static str>>,
    open_in: Entity<SelectState<&'static str>>,
    folder: Entity<InputState>,
    pick_theme: Entity<SelectState<&'static str>>,
    sign_in: Entity<SelectState<&'static str>>,
    open_list: Entity<SelectState<&'static str>>,
    name: Entity<InputState>,
    port: Entity<InputState>,
    key: Entity<InputState>,
    program: Entity<InputState>,
    /// Where the lone section drew last frame, so a capture can be cut to it.
    drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    _picked: Subscription,
}

fn list(items: &[(&'static str, &'static str)], picked: &'static str, cx: &mut Context<Gallery>) -> Entity<SelectState<&'static str>> {
    let items = items.iter().map(|&(value, label)| SelectItem::new(value, label)).collect();
    cx.new(|_| SelectState::new(items, Some(&picked)))
}

impl Gallery {
    pub fn new(only: Option<Only>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let themes: Vec<(&'static str, &'static str)> = theme::THEMES.iter().map(|t| (t.id, t.word)).collect();
        let theme = list(&themes, theme::current(cx).id, cx);
        let picked = cx.subscribe_in(&theme, window, |this, _, event: &SelectEvent<&'static str>, window, cx| {
            let SelectEvent::Confirm(id) = event;
            this.pick_theme(id, window, cx);
        });
        let field = |value: &str, placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            let (value, placeholder) = (value.to_owned(), placeholder.to_owned());
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value))
        };
        let port = field("4180", "", window, cx);
        port.update(cx, |state, cx| state.focus(window, cx));
        Gallery {
            only,
            theme,
            open_in: list(&[("same", "The same window"), ("new", "A new window")], "same", cx),
            folder: field("~/code", "", window, cx),
            pick_theme: list(&[("paper", "Paper"), ("graphite", "Graphite")], "paper", cx),
            open_list: cx.new(|cx| {
                let items = vec![
                    SelectItem::new("same", "The same window").glyph(Glyph::AppWindow),
                    SelectItem::new("new", "A new window").glyph(Glyph::PanelsTopLeft),
                    SelectItem::new("split", "A split beside it").glyph(Glyph::PanelLeft),
                ];
                let mut state = SelectState::new(items, Some(&"new"));
                state.set_open(true, cx);
                state
            }),
            sign_in: list(&[("copy", "Copy the key"), ("sign-in", "Sign in there")], "copy", cx),
            name: field("", "zingzy's MacBook Pro", window, cx),
            port,
            key: field("sk-live-0000", "Solari API key", window, cx),
            program: field("/usr/local/bin/claude", "claude", window, cx),
            drawn: Rc::default(),
            _picked: picked,
        }
    }

    /// The theme picker at the gallery's top.
    pub fn picker(&self) -> &Entity<SelectState<&'static str>> {
        &self.theme
    }

    /// The lone section's or piece's box as it was last drawn.
    pub fn drawn(&self) -> Option<Bounds<Pixels>> {
        self.drawn.get()
    }

    /// What the theme picker does: every window restyles in the theme `id` names.
    pub fn pick_theme(&mut self, id: &str, _: &mut Window, cx: &mut Context<Self>) {
        theme::set_theme(id, cx);
        cx.notify();
    }

    fn open_in(&self) -> Row {
        Row::new("open-in", "Open threads in")
            .note("Where a thread opens when you pick it in the sidebar.")
            .control(Select::new(&self.open_in))
    }

    fn default_folder(&self) -> Row {
        Row::new("default-folder", "Default folder")
            .note("New threads start here unless a project says otherwise.")
            .control(Field::new(&self.folder))
    }

    fn macbook(cx: &App) -> Row {
        Row::new("macbook", "zingzy's MacBook Pro")
            .lead(glyph_frame(Glyph::Monitor, cx))
            .note("8 cores, 16 GB")
            .fact("Ready")
            .on_open(|_, _, _| {})
    }

    fn general(&self) -> Card {
        Card::new("general")
            .head("General")
            .item(self.open_in())
            .item(self.default_folder())
            .item(Line::new("version", "Version").value("0.42.1"))
    }

    fn computers(cx: &App) -> Card {
        Card::new("computers")
            .head("Computers")
            .lede("Each computer runs its own agents and keeps its own sign-ins.")
            .item(Self::macbook(cx))
            .item(
                Row::new("solari", "Solari")
                    .lead(glyph_frame(Glyph::Cloud, cx))
                    .note("2 of 2 machines running")
                    .fact("Full")
                    .on_open(|_, _, _| {}),
            )
    }

    fn send_with_enter() -> Row {
        Row::new("send-with-enter", "Send with Enter")
            .note("Shift and Enter adds a line.")
            .control(Button::outline("change", "Change").xs())
    }

    fn spoo_landing(cx: &App) -> Row {
        Row::new("spoo-landing", "spoo-landing").lead(glyph_frame(Glyph::Folder, cx)).note("~/code/spoo-landing").value("3 threads")
    }

    fn older_wsp() -> Row {
        Row::new("older-wsp", "Older wsp").warning().note("This computer runs 0.39.0; threads still start there.")
    }

    fn new_thread() -> Line {
        Line::new("new-thread", "New thread").keys(KeyCaps::new(&[&["Ctrl", "T"]]))
    }

    fn switch_thread() -> Line {
        Line::new("switch-thread", "Switch thread").keys(KeyCaps::new(&[&["Ctrl", "1"], &["Ctrl", "9"]]).joiner("to"))
    }

    fn disk() -> Line {
        Line::new("disk", "Disk").value("9.4 GB")
    }

    fn no_servers() -> Line {
        Line::new("no-servers", "No servers yet.").empty()
    }

    fn version_line() -> Line {
        Line::new("wsp-version", "Version").fact(["wsp 0.42.1", "daemon 0.42.1"]).hover("What this computer runs.")
    }

    fn send_with() -> Row {
        Row::new("send-with", "Send with")
            .note("What sends a message from the composer.")
            .on_reset(|_, _, _| {})
            .control(Button::outline("send-with-change", "Change").xs())
    }

    fn default_mark(cx: &App) -> Row {
        Row::new("default-computer", "zingzy's MacBook Pro").lead(glyph_frame(Glyph::Laptop, cx)).mark("Default").note("This computer")
    }

    fn remote() -> Row {
        Row::new("remote", "Repository")
            .note("git@github.com:wsp-labs/wsp.git")
            .mono()
            .control(Button::outline("remote-open", "Open").xs().glyph(Glyph::ExternalLink))
    }

    fn command() -> Row {
        Row::new("program", "Program").note("/usr/local/bin/claude --dangerously-skip-permissions --model opus --verbose").clip()
    }

    fn chip_row() -> Row {
        Row::new("shape", "Solari 8x16").chips(vec![
            Chip::new("8 cores").glyph(Glyph::Cpu),
            Chip::new("16 GB").glyph(Glyph::HardDrive),
            Chip::new("146 GB disk").glyph(Glyph::Container),
        ])
    }

    fn spaced_row() -> Row {
        Row::new("spaced", "zingzy's MacBook Pro").note(["8 cores", "16 GB", "macOS 15.5"]).fact("Ready")
    }

    fn open_control(cx: &App) -> Row {
        Row::new("open-control", "Claude Code")
            .lead(glyph_frame(Glyph::Bot, cx))
            .note("Signed in as zingzy")
            .control(Button::outline("open-control-sign-out", "Sign out").xs())
            .on_open(|_, _, _| {})
    }

    fn head_row(cx: &App) -> HeadRow {
        HeadRow::new("computer-head", glyph(Glyph::Laptop, cx), "zingzy's MacBook Pro")
            .mark(VersionFact::new(vec!["wsp 0.42.1".into(), "daemon 0.42.1".into()]))
            .line("8 cores, 16 GB, macOS 15.5")
    }

    fn grid(cx: &App) -> Grid {
        let row =
            |id: &'static str, g: Glyph, name: &'static str, cores: &'static str, memory: &'static str, threads: &'static str, cx: &App| {
                GridRow::new(id, LIST_COLUMNS)
                    .on_open(|_, _, _| {})
                    .cell(GridName::new(glyph_frame(g, cx), name).fact(vec!["wsp 0.42.1".into()]))
                    .cell(num(cores, cx))
                    .cell(num(memory, cx))
                    .cell(num(threads, cx))
                    .cell(StateCell::word(format!("{id}-state"), "Ready").why("Answering, and up to date."))
                    .cell(chevron(cx))
            };
        Grid::new("computers-grid")
            .head(GridHead::new(
                Some(LIST_COLUMNS),
                vec![
                    HeadCell::new("Computers"),
                    HeadCell::new("Cores").num(),
                    HeadCell::new("Memory").num(),
                    HeadCell::new("Threads").num(),
                ],
            ))
            .row(row("macbook-grid", Glyph::Laptop, "zingzy's MacBook Pro", "8", "16 GB", "3", cx))
            .row(row("solari-grid", Glyph::Server, "Solari", "8", "16 GB", "1", cx))
    }

    fn crumbs(cx: &App) -> impl IntoElement {
        let muted = cx.palette().muted_foreground;
        h_flex().gap_2().text_size(px(13.)).child(div().text_color(muted).child("Computers")).child(slash(cx)).child("Solari")
    }

    fn lockup(cx: &App) -> impl IntoElement {
        gpui_kit::svg().path(LOCKUP).w(px(51.)).h(px(14.)).text_color(cx.palette().foreground)
    }

    fn plan_alerts() -> Row {
        Row::new("plan-alerts", "Plan alerts").note("A sound when a plan waits for you.").control(Switch::new("plan-alerts-switch", true))
    }

    fn send_keys() -> Row {
        Row::new("send-keys", "Send with").note("What sends a message from the composer.").control(Self::segmented())
    }

    fn segmented() -> SegmentedControl<&'static str> {
        SegmentedControl::new("send-with-segments", "enter", vec![Segment::new("enter", "Enter"), Segment::new("mod-enter", "Ctrl Enter")])
    }

    fn tabs() -> SegmentedControl<&'static str> {
        SegmentedControl::new(
            "usage-tabs",
            "spend",
            vec![
                Segment::new("spend", "Spend"),
                Segment::new("tokens", "Tokens"),
                Segment::new("time", "Time").held("No timings on this computer yet."),
            ],
        )
        .large()
    }

    fn choices() -> [Choice; 2] {
        [
            Choice::new("copy-keys", "Copy the keys to Solari", true)
                .glyph(Glyph::KeyRound)
                .note("GITHUB_TOKEN and LINEAR_API_KEY go with the servers."),
            Choice::new("leave-keys", "Leave the keys on zingzy's MacBook Pro", false)
                .glyph(Glyph::KeyRound)
                .note("Those servers stay off on Solari."),
        ]
    }

    fn picks() -> [PickRow; 3] {
        [
            PickRow::new("claude", "Claude Code", true).glyph(Glyph::Terminal).note("Signed in as zingzy"),
            PickRow::new("codex", "Codex", true).glyph(Glyph::Terminal).note("Copy the key"),
            PickRow::new("cursor", "Cursor", false).glyph(Glyph::Terminal).note("Not installed here"),
        ]
    }

    fn copy_row() -> CopyRow {
        CopyRow::new("command", "curl -fsSL https://wsp.sh/install | sh -s -- --token 7f3a9c").label("Command")
    }

    fn remove_project() -> AlertDialog {
        AlertDialog::new("Remove spoo-landing?")
            .sentence("Its 3 threads stay on zingzy's MacBook Pro; the folder is not touched.")
            .act(Button::outline("remove-cancel", "Cancel").neutral_ring())
            .act(Button::destructive("remove-confirm", "Remove"))
    }

    fn sheet(&self) -> AlertDialog {
        AlertDialog::new("Program")
            .sentence("The program run in Claude Code's place on Solari: a path, or a word on its PATH.")
            .child(Field::new(&self.program).sheet().invalid(true))
            .child(Refusal::new().said("No program at /usr/local/bin/claude on Solari.").fix("Check the path."))
            .act(Button::outline("sheet-cancel", "Cancel"))
            .act(Button::primary("sheet-save", "Save"))
    }

    fn locked() -> Card {
        Card::new("locked").locked(true).head("This month").item(Line::new("spend", "Spend").value("$41.20"))
    }

    fn head_link() -> Card {
        Card::new("head-link").head("General").on_head_click(|_, _, _| {}).item(Line::new("theme-line", "Theme").value("Paper"))
    }

    fn dialog(cx: &App) -> Dialog {
        let claude = PickRow::new("d-claude", "Claude Code", true).glyph(Glyph::Terminal).note("Signed in as zingzy");
        let cursor = PickRow::new("d-cursor", "Cursor", false).glyph(Glyph::Terminal).note("Not installed here");
        Dialog::new("Add a computer")
            .count(2, 5)
            .line("Pick what goes on Solari.")
            .card(Card::new("agents").item(claude).item(cursor))
            .child(Self::copy_row())
            .state(quiet("Saved, you can finish later", cx))
            .act(Button::outline("d-back", "Back"))
            .act(Button::primary("d-next", "Next"))
    }

    fn marks() -> Card {
        let mark = |id: &'static str, label: &'static str, status: Status| Line::new(id, label).right(StatusMark::new(status));
        Card::new("marks")
            .item(mark("needs-you", "Needs you", Status::NeedsYou))
            .item(mark("working", "Working", Status::Working("1h 5m".into())))
            .item(mark("failed", "Failed", Status::Failed))
            .item(mark("done", "Done", Status::Done))
            .item(mark("waiting", "Waiting", Status::Waiting))
            .item(mark("read", "Read", Status::Read("14m".into())))
    }

    fn steps() -> Card {
        Card::new("steps")
            .item(StepRow::new("not-started", Step::NotStarted, "Not started"))
            .item(StepRow::new("running", Step::Running, "Installing Claude Code"))
            .item(StepRow::new("skipped", Step::Skipped, "Set aside"))
            .item(StepRow::new("step-done", Step::Done, "Done"))
            .item(StepRow::new("step-failed", Step::Failed, "Failed"))
    }

    fn tooltip(window: &mut Window, cx: &mut App) -> gpui_kit::AnyView {
        shortcut_tooltip_view("Settings".into(), vec!["Ctrl".into(), ",".into()], window, cx)
    }

    fn card(&self, cx: &App) -> impl IntoElement {
        v_flex().gap(size::PAGE_GAP).child(self.general()).child(Self::computers(cx))
    }

    fn lists(&self, cx: &App) -> impl IntoElement {
        v_flex()
            .gap(size::PAGE_GAP)
            .child(Card::new("head").item(Self::head_row(cx)).item(Self::version_line()))
            .child(Self::grid(cx))
            .child(Card::new("loading-rows").head("Devices").body(RowSkeleton::new("devices-loading")))
            .child(h_flex().gap_6().child(Self::crumbs(cx)).child(Self::lockup(cx)))
    }

    fn rows(&self, cx: &App) -> impl IntoElement {
        Card::new("rows")
            .item(Self::send_with())
            .item(Self::default_mark(cx))
            .item(Self::remote())
            .item(Self::command())
            .item(Self::chip_row())
            .item(Self::spaced_row())
            .item(Self::open_control(cx))
            .item(Self::send_with_enter())
            .item(Self::spoo_landing(cx))
            .item(Self::older_wsp())
            .item(Self::new_thread())
            .item(Self::switch_thread())
            .item(Self::disk())
            .item(Self::no_servers())
    }

    fn controls(&self) -> impl IntoElement {
        let line = |children: Vec<Button>| h_flex().gap_3().children(children);
        v_flex()
            .gap(size::PAGE_GAP)
            .child(
                Card::new("select-and-field")
                    .head("Select and field")
                    .item(Row::new("theme", "Theme").note("Light side.").control(Select::new(&self.pick_theme)))
                    .item(
                        Row::new("sign-in", "Sign-in").note("How the agent signs in there.").control(Select::new(&self.sign_in).held(true)),
                    )
                    .item(Row::new("name", "Name").note("What the sidebar calls this computer.").control(Field::new(&self.name)))
                    .item(Row::new("port", "Port").note("The host listens here.").control(Field::new(&self.port)))
                    .item(Row::new("key", "Key").note("Solari refused it.").control(Field::new(&self.key).invalid(true)))
                    .item(Row::new("held-folder", "Folder").note("Waits on the computer.").control(Field::new(&self.folder).held(true)))
                    .item(
                        Row::new("revoke", "zingzy's iPhone")
                            .note("Signed in 3 days ago")
                            .control(Button::danger("revoke-phone", "Revoke").xs()),
                    ),
            )
            .child(
                Card::new("toggles")
                    .head("Switch and segmented control")
                    .item(Self::plan_alerts())
                    .item(Row::new("keep-awake", "Keep zingzy's MacBook Pro awake").control(Switch::new("keep-awake-switch", false)))
                    .item(
                        Row::new("login-start", "Open at login")
                            .note("Waits on the system.")
                            .control(Switch::new("login-switch", true).held(true)),
                    )
                    .item(Self::send_keys())
                    .under(Self::tabs()),
            )
            .child(Card::new("choices").head("Picks, one of many").items(Self::choices()))
            .child(Card::new("picks").head("Picks, many of many").items(Self::picks()))
            .child(
                v_flex()
                    .gap_3()
                    .child(line(vec![
                        Button::outline("copy", "Copy").xs().glyph(Glyph::Copy),
                        Button::outline("try-again", "Try again").xs().glyph(Glyph::RotateCcw),
                        Button::outline("retry", "Retry").xs().held(true),
                    ]))
                    .child(line(vec![
                        Button::outline("back", "Back"),
                        Button::primary("next", "Next"),
                        Button::primary("next-held", "Next").held(true),
                        Button::text("skip", "Skip for now"),
                        Button::ghost("restore", "Restore defaults").xs().glyph(Glyph::RotateCcw),
                    ]))
                    .child(line(vec![Button::add("add-computer", "Add a computer"), Button::icon("settings", Glyph::Settings)])),
            )
    }

    fn status(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let crab = Crab::new(cx.palette().status_working);
        let tooltip = Self::tooltip(window, cx);
        v_flex()
            .gap(size::PAGE_GAP)
            .child(v_flex().gap_3().child(section_head("Status mark", cx)).child(Self::marks()))
            .child(v_flex().gap_3().child(section_head("Steps", cx)).child(Self::steps()))
            .child(
                Card::new("loading")
                    .head("Loading and the crab")
                    .item(Line::new("loading", "Loading").right(spinner(cx)))
                    .item(Line::new("crab", "Working").right(crab))
                    .under(quiet("The crab is Whimsy Loaders' pixel crab, by Sasha.", cx)),
            )
            .child(Card::new("tooltip").head("Shortcut in a tooltip").item(CardItem::any(
                v_flex().items_center().gap_1().py(px(14.)).child(tooltip).child(Button::icon("tooltip-settings", Glyph::Settings)),
            )))
    }

    fn section(&self, section: Section, window: &mut Window, cx: &mut App) -> AnyElement {
        match section {
            Section::Card => self.card(cx).into_any_element(),
            Section::Rows => self.rows(cx).into_any_element(),
            Section::Controls => self.controls().into_any_element(),
            Section::Dialog => v_flex()
                .gap(size::PAGE_GAP)
                .child(Self::dialog(cx))
                .child(Self::remove_project())
                .child(self.sheet())
                .child(Self::locked())
                .child(Self::head_link())
                .into_any_element(),
            Section::Status => self.status(window, cx).into_any_element(),
            Section::Lists => self.lists(cx).into_any_element(),
        }
    }

    fn piece(&self, piece: Piece, window: &mut Window, cx: &mut App) -> AnyElement {
        let one = |item: CardItem| Card::new("piece").item(item).into_any_element();
        match piece {
            Piece::SectionHead => section_head("General", cx).into_any_element(),
            Piece::Card => self.general().into_any_element(),
            Piece::CardLede => Self::computers(cx).into_any_element(),
            Piece::RowSelect => one(self.open_in().into()),
            Piece::RowField => one(self.default_folder().into()),
            Piece::RowButton => one(Self::send_with_enter().into()),
            Piece::RowLead => one(Self::spoo_landing(cx).into()),
            Piece::RowOpen => one(Self::macbook(cx).into()),
            Piece::RowWarning => one(Self::older_wsp().into()),
            Piece::LineValue => one(Self::disk().into()),
            Piece::LineKeys => one(Self::new_thread().into()),
            Piece::LineRange => one(Self::switch_thread().into()),
            Piece::LineEmpty => one(Self::no_servers().into()),
            Piece::LineFact => one(Self::version_line().into()),
            Piece::RowReset => one(Self::send_with().into()),
            Piece::RowMark => one(Self::default_mark(cx).into()),
            Piece::RowMono => one(Self::remote().into()),
            Piece::RowClip => one(Self::command().into()),
            Piece::RowChips => one(Self::chip_row().into()),
            Piece::RowSpaced => one(Self::spaced_row().into()),
            Piece::RowOpenControl => one(Self::open_control(cx).into()),
            Piece::HeadRow => one(Self::head_row(cx).into()),
            Piece::Grid => Self::grid(cx).into_any_element(),
            Piece::RowSkeleton => RowSkeleton::new("row-skeleton").into_any_element(),
            Piece::Skeleton => Skeleton::new(px(160.), px(14.)).into_any_element(),
            Piece::Crumbs => Self::crumbs(cx).into_any_element(),
            Piece::Lockup => Self::lockup(cx).into_any_element(),
            Piece::GlyphFrame => glyph_frame(Glyph::Monitor, cx).into_any_element(),
            Piece::Select => Select::new(&self.open_in).into_any_element(),
            Piece::SelectOpen => {
                div().w(px(240.)).h(px(140.)).pt(px(44.)).pl(px(8.)).child(Select::new(&self.open_list)).into_any_element()
            }
            Piece::SelectHeld => Select::new(&self.sign_in).held(true).into_any_element(),
            Piece::Field => Field::new(&self.folder).into_any_element(),
            Piece::FieldEmpty => Field::new(&self.name).into_any_element(),
            Piece::FieldFocused => Field::new(&self.port).into_any_element(),
            Piece::RowSwitch => one(Self::plan_alerts().into()),
            Piece::RowSegmented => one(Self::send_keys().into()),
            Piece::Switch => Switch::new("switch", true).into_any_element(),
            Piece::SwitchOff => Switch::new("switch", false).into_any_element(),
            Piece::SwitchHeld => Switch::new("switch", true).held(true).into_any_element(),
            Piece::Segmented => Self::segmented().into_any_element(),
            Piece::SegmentedLarge => Self::tabs().into_any_element(),
            Piece::Choice => Card::new("choices").items(Self::choices()).into_any_element(),
            Piece::Pick => Card::new("picks").items(Self::picks()).into_any_element(),
            Piece::ButtonXs => Button::outline("copy", "Copy").xs().glyph(Glyph::Copy).into_any_element(),
            Piece::ButtonXsHeld => Button::outline("retry", "Retry").xs().held(true).into_any_element(),
            Piece::ButtonOutline => Button::outline("back", "Back").into_any_element(),
            Piece::ButtonPrimary => Button::primary("next", "Next").into_any_element(),
            Piece::ButtonPrimaryHeld => Button::primary("next", "Next").held(true).into_any_element(),
            Piece::ButtonText => Button::text("skip", "Skip for now").into_any_element(),
            Piece::ButtonGhost => Button::ghost("restore", "Restore defaults").xs().glyph(Glyph::RotateCcw).into_any_element(),
            Piece::ButtonAdd => Button::add("add-computer", "Add a computer").into_any_element(),
            Piece::ButtonIcon => Button::icon("settings", Glyph::Settings).into_any_element(),
            Piece::Keycaps => KeyCaps::new(&[&["Ctrl", "T"]]).into_any_element(),
            Piece::Dialog => Self::dialog(cx).into_any_element(),
            Piece::AlertDialog => Self::remove_project().into_any_element(),
            Piece::Sheet => self.sheet().into_any_element(),
            Piece::Refusal => Refusal::new().said("Solari refused the key.").fix("Paste a new one.").into_any_element(),
            Piece::FieldHeld => Field::new(&self.folder).held(true).into_any_element(),
            Piece::FieldInvalid => Field::new(&self.key).invalid(true).into_any_element(),
            Piece::ButtonDanger => Button::danger("revoke", "Revoke").xs().into_any_element(),
            Piece::ButtonDestructive => Button::destructive("remove", "Remove").into_any_element(),
            Piece::CardLocked => Self::locked().into_any_element(),
            Piece::CardHeadLink => Self::head_link().into_any_element(),
            Piece::CopyRow => div().w(size::DIALOG_WIDTH - px(40.)).child(Self::copy_row().on(cx.palette().background)).into_any_element(),
            Piece::Status => Self::marks().into_any_element(),
            Piece::Steps => Self::steps().into_any_element(),
            Piece::Spinner => spinner(cx).into_any_element(),
            Piece::Crab => Crab::new(cx.palette().status_working).into_any_element(),
            Piece::Tooltip => Self::tooltip(window, cx).into_any_element(),
        }
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let page = div().size_full().bg(p.background).text_color(p.foreground);
        if let Some(only) = self.only {
            let drawn = self.drawn.clone();
            let probe = canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {}).absolute().top_0().left_0().size_full();
            let lone = match only {
                Only::Section(section) => {
                    div().relative().w(CONTENT + PAD * 2.).p(PAD).child(self.section(section, window, cx)).child(probe)
                }
                Only::Piece(piece) => {
                    let measured = div().relative().when(piece.wide(), |el| el.w_full()).child(self.piece(piece, window, cx)).child(probe);
                    div().w(CONTENT + PAD * 2.).p(PAD).child(v_flex().items_start().child(measured))
                }
            };
            return page.child(lone).into_any_element();
        }
        let sections: Vec<_> = Section::ALL
            .iter()
            .map(|&s| v_flex().gap_3().child(crate::ui::section_head(s.title(), cx)).child(self.section(s, window, cx)))
            .collect();
        let picker = Card::new("theme").head("Theme").item(
            Row::new("gallery-theme", "Theme")
                .note("Every piece below restyles at once.")
                .fact(if theme::current(cx).side == theme::Side::Dark { "Dark side" } else { "Light side" })
                .control(Select::new(&self.theme)),
        );
        page.child(
            div()
                .id("gallery")
                .size_full()
                .overflow_y_scroll()
                .child(v_flex().w(CONTENT + PAD * 2.).p(PAD).gap(size::PAGE_GAP).child(picker).children(sections)),
        )
        .into_any_element()
    }
}
