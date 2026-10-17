// SPDX-License-Identifier: AGPL-3.0-only
//! `desktop-gpui --gallery`: every shared piece in every state, in the current theme, with the theme picker at the top.
//! Every later screen is checked against it. Its words and pieces match a capture of the web app's own components.

use gpui_kit::component::input::InputState;
use gpui_kit::component::searchable_list::SearchableVec;
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::{h_flex, v_flex, IndexPath};
use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::{canvas, div, prelude::*, px, AnyElement, App, Bounds, Context, Entity, Pixels, SharedString, Subscription, Window};

use crate::ui::dialog::shortcut_tooltip_view;
use crate::ui::{
    glyph_frame, quiet, section_head, size, spinner, theme, ActivePalette, Button, Card, CardItem, Choice, CopyRow, Crab, Dialog, Field,
    Glyph, KeyCaps, Line, PickRow, Row, Select, SelectList, Status, StatusMark, Step, StepRow,
};

/// The width every section's content takes: a Settings page's 640 px column.
pub const CONTENT: Pixels = px(640.);
/// The padding round a section.
pub const PAD: Pixels = px(24.);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Card,
    Rows,
    Controls,
    Dialog,
    Status,
}

impl Section {
    pub const ALL: [Section; 5] = [Section::Card, Section::Rows, Section::Controls, Section::Dialog, Section::Status];

    pub fn name(self) -> &'static str {
        match self {
            Section::Card => "card",
            Section::Rows => "rows",
            Section::Controls => "controls",
            Section::Dialog => "dialog",
            Section::Status => "status",
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
    GlyphFrame "glyph-frame" false,
    Select "select" false,
    SelectHeld "select-held" false,
    Field "field" false,
    FieldEmpty "field-empty" false,
    FieldFocused "field-focused" false,
    Choice "choice" true,
    Pick "pick" true,
    ButtonXs "button-xs" false,
    ButtonXsHeld "button-xs-held" false,
    ButtonOutline "button-outline" false,
    ButtonPrimary "button-primary" false,
    ButtonPrimaryHeld "button-primary-held" false,
    ButtonText "button-text" false,
    ButtonAdd "button-add" false,
    ButtonIcon "button-icon" false,
    Keycaps "keycaps" false,
    Dialog "dialog" false,
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
    theme: Entity<SelectList>,
    open_in: Entity<SelectList>,
    folder: Entity<InputState>,
    pick_theme: Entity<SelectList>,
    sign_in: Entity<SelectList>,
    name: Entity<InputState>,
    port: Entity<InputState>,
    /// Where the lone section drew last frame, so a capture can be cut to it.
    drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    _picked: Subscription,
}

fn list(items: &[&'static str], picked: usize, window: &mut Window, cx: &mut Context<Gallery>) -> Entity<SelectList> {
    let items = SearchableVec::new(items.iter().map(|s| SharedString::new_static(s)).collect::<Vec<_>>());
    cx.new(|cx| SelectList::new(items, Some(IndexPath::new(picked)), window, cx))
}

impl Gallery {
    pub fn new(only: Option<Only>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let words: Vec<&'static str> = theme::THEMES.iter().map(|t| t.word).collect();
        let current = theme::THEMES.iter().position(|t| t.id == theme::current(cx).id).unwrap_or(0);
        let theme = list(&words, current, window, cx);
        let picked = cx.subscribe_in(&theme, window, |this, _, event: &SelectEvent<SearchableVec<SharedString>>, window, cx| {
            if let SelectEvent::Confirm(Some(word)) = event {
                if let Some(entry) = theme::THEMES.iter().find(|t| t.word == word.as_ref()) {
                    this.pick_theme(entry.id, window, cx);
                }
            }
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
            open_in: list(&["The same window", "A new window"], 0, window, cx),
            folder: field("~/code", "", window, cx),
            pick_theme: list(&["Paper", "Graphite"], 0, window, cx),
            sign_in: list(&["Copy the key", "Sign in there"], 0, window, cx),
            name: field("", "zingzy's MacBook Pro", window, cx),
            port,
            drawn: Rc::default(),
            _picked: picked,
        }
    }

    /// The theme picker at the gallery's top.
    pub fn picker(&self) -> &Entity<SelectList> {
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
        Card::new().head("General").item(self.open_in()).item(self.default_folder()).item(Line::new("version", "Version").value("0.42.1"))
    }

    fn computers(cx: &App) -> Card {
        Card::new().head("Computers").lede("Each computer runs its own agents and keeps its own sign-ins.").item(Self::macbook(cx)).item(
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

    fn dialog(cx: &App) -> Dialog {
        let [claude, _, cursor] = Self::picks();
        Dialog::new("Add a computer")
            .count(2, 5)
            .line("Pick what goes on Solari.")
            .card(Card::new().item(claude).item(cursor))
            .child(Self::copy_row())
            .state(quiet("Saved, you can finish later", cx))
            .act(Button::outline("d-back", "Back"))
            .act(Button::primary("d-next", "Next"))
    }

    fn marks() -> Card {
        let mark = |id: &'static str, label: &'static str, status: Status| Line::new(id, label).right(StatusMark::new(status));
        Card::new()
            .item(mark("needs-you", "Needs you", Status::NeedsYou))
            .item(mark("working", "Working", Status::Working("1h 5m".into())))
            .item(mark("failed", "Failed", Status::Failed))
            .item(mark("done", "Done", Status::Done))
            .item(mark("waiting", "Waiting", Status::Waiting))
            .item(mark("read", "Read", Status::Read("14m".into())))
    }

    fn steps() -> Card {
        Card::new()
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

    fn rows(&self, cx: &App) -> impl IntoElement {
        Card::new()
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
                Card::new()
                    .head("Select and field")
                    .item(Row::new("theme", "Theme").note("Light side.").control(Select::new(&self.pick_theme)))
                    .item(
                        Row::new("sign-in", "Sign-in").note("How the agent signs in there.").control(Select::new(&self.sign_in).held(true)),
                    )
                    .item(Row::new("name", "Name").note("What the sidebar calls this computer.").control(Field::new(&self.name)))
                    .item(Row::new("port", "Port").note("The host listens here.").control(Field::new(&self.port))),
            )
            .child(Card::new().head("Picks, one of many").items(Self::choices()))
            .child(Card::new().head("Picks, many of many").items(Self::picks()))
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
                Card::new()
                    .head("Loading and the crab")
                    .item(Line::new("loading", "Loading").right(spinner(cx)))
                    .item(Line::new("crab", "Working").right(crab))
                    .under(quiet("The crab is Whimsy Loaders' pixel crab, by Sasha.", cx)),
            )
            .child(Card::new().head("Shortcut in a tooltip").item(CardItem::any(
                v_flex().items_center().gap_1().py(px(14.)).child(tooltip).child(Button::icon("tooltip-settings", Glyph::Settings)),
            )))
    }

    fn section(&self, section: Section, window: &mut Window, cx: &mut App) -> AnyElement {
        match section {
            Section::Card => self.card(cx).into_any_element(),
            Section::Rows => self.rows(cx).into_any_element(),
            Section::Controls => self.controls().into_any_element(),
            Section::Dialog => Self::dialog(cx).into_any_element(),
            Section::Status => self.status(window, cx).into_any_element(),
        }
    }

    fn piece(&self, piece: Piece, window: &mut Window, cx: &mut App) -> AnyElement {
        let one = |item: CardItem| Card::new().item(item).into_any_element();
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
            Piece::GlyphFrame => glyph_frame(Glyph::Monitor, cx).into_any_element(),
            Piece::Select => Select::new(&self.open_in).into_any_element(),
            Piece::SelectHeld => Select::new(&self.sign_in).held(true).into_any_element(),
            Piece::Field => Field::new(&self.folder).into_any_element(),
            Piece::FieldEmpty => Field::new(&self.name).into_any_element(),
            Piece::FieldFocused => Field::new(&self.port).into_any_element(),
            Piece::Choice => Card::new().items(Self::choices()).into_any_element(),
            Piece::Pick => Card::new().items(Self::picks()).into_any_element(),
            Piece::ButtonXs => Button::outline("copy", "Copy").xs().glyph(Glyph::Copy).into_any_element(),
            Piece::ButtonXsHeld => Button::outline("retry", "Retry").xs().held(true).into_any_element(),
            Piece::ButtonOutline => Button::outline("back", "Back").into_any_element(),
            Piece::ButtonPrimary => Button::primary("next", "Next").into_any_element(),
            Piece::ButtonPrimaryHeld => Button::primary("next", "Next").held(true).into_any_element(),
            Piece::ButtonText => Button::text("skip", "Skip for now").into_any_element(),
            Piece::ButtonAdd => Button::add("add-computer", "Add a computer").into_any_element(),
            Piece::ButtonIcon => Button::icon("settings", Glyph::Settings).into_any_element(),
            Piece::Keycaps => KeyCaps::new(&[&["Ctrl", "T"]]).into_any_element(),
            Piece::Dialog => Self::dialog(cx).into_any_element(),
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
        let picker = Card::new().head("Theme").item(
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
