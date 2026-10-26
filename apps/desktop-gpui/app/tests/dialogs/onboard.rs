// SPDX-License-Identifier: AGPL-3.0-only
//! Onboarding, the starting window and first run's states for `dialogs_shots`. They name the computer as the app reads
//! it, so a capture beside a baseline shot on the same computer names the same one.

use desktop_gpui::screens::dialogs::first_run::{FirstRun, Holds};
use desktop_gpui::screens::dialogs::onboarding::{self, Onboarding, Refused, Step};
use desktop_gpui::screens::dialogs::starting::StartingView;
use desktop_gpui::screens::dialogs::startup::{self, Agent};
use desktop_gpui::ui::{catalog, ActivePalette};
use gpui_kit::component::h_flex;
use gpui_kit::{div, prelude::*, px, AnyView, App, Context, Window};

use super::{page, State};

static HERE: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| startup::computer_name().unwrap_or_else(|| "studio".into()));
const ORDER: [&str; 11] = ["claude", "codex", "gemini", "opencode", "pi", "hermes", "crush", "qwen", "goose", "amp", "cursor"];

fn agents(found: &[&str]) -> Vec<Agent> {
    ORDER.iter().map(|id| Agent { id: (*id).into(), name: catalog::name(id), found: found.contains(id), installs: false }).collect()
}

fn staged(step: Step, scan: Result<Vec<Agent>, String>, refused: Option<Refused>, window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| Onboarding::staged(step, Some(HERE.as_str()), scan, refused, window, cx)).into()
}

fn tools_refused() -> Refused {
    Refused {
        happened: onboarding::tools_refused(&["Claude Code".into()]),
        fix: onboarding::TOOLS_FIX.into(),
        said: Some("claude: EACCES: permission denied, open '~/.claude.json'".into()),
    }
}

/// First run where the shell draws it: right of the 256 px sidebar, under the 36 px header row.
struct Centre {
    first_run: AnyView,
}

impl Render for Centre {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette();
        h_flex()
            .size_full()
            .bg(p.background)
            .child(div().flex_none().w(px(256.)).h_full().bg(p.sidebar_canvas).border_r_1().border_color(p.border))
            .child(div().flex_1().h_full().pt(px(36.)).child(self.first_run.clone()))
    }
}

pub fn states() -> Vec<State> {
    vec![
        State::new("W2-DLG-032", |window, cx| staged(Step::Welcome, Ok(agents(&["claude"])), None, window, cx)),
        State::new("W2-DLG-033", |window, cx| staged(Step::Agents, Ok(agents(&["claude"])), None, window, cx)),
        State::new("W2-DLG-034", |window, cx| {
            cx.new(|cx| {
                let mut screen = Onboarding::staged(Step::Agents, Some(HERE.as_str()), Ok(agents(&["claude"])), None, window, cx);
                screen.toggle("claude", cx);
                screen
            })
            .into()
        }),
        State::new("W2-DLG-035", |window, cx| staged(Step::Agents, Ok(agents(&[])), None, window, cx)),
        State::new("W2-DLG-036", |window, cx| staged(Step::Agents, Ok(agents(&["claude"])), Some(tools_refused()), window, cx)),
        State::new("W2-DLG-037", |window, cx| {
            staged(Step::Agents, Err("agents.read: the login PATH could not be read".into()), None, window, cx)
        }),
        State::new("W2-DLG-037-shell", |window, cx| {
            let refused = Refused {
                happened: onboarding::shell_refused(Some(HERE.as_str())),
                fix: onboarding::SHELL_FIX.into(),
                said: Some("the host closed the socket".into()),
            };
            staged(Step::Agents, Ok(agents(&["claude"])), Some(refused), window, cx)
        }),
        State::new("W2-DLG-038", |_, cx| cx.new(|_| StartingView::new()).into()),
        State::new("W2-DLG-040", |window, cx| {
            let behind = page(window, cx);
            let holds = Holds { projects: 0, workspaces: 0, here: HERE.clone() };
            let first_run = cx.new(|_| FirstRun::staged(holds, false, behind)).into();
            cx.new(|_| Centre { first_run }).into()
        }),
        State::new("W2-DLG-040-usage", |window, cx| {
            let behind = page(window, cx);
            let holds = Holds { projects: 0, workspaces: 0, here: HERE.clone() };
            let first_run = cx.new(|_| FirstRun::staged(holds, true, behind)).into();
            cx.new(|_| Centre { first_run }).into()
        }),
    ]
}
