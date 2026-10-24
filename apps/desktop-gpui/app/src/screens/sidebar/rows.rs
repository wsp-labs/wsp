// SPDX-License-Identifier: AGPL-3.0-only
//! What the list draws besides the threads' rows: a send in flight (`ThreadLaunchTile`), a workspace being made
//! (`CreationTile`), a cloud workspace with no thread yet (`WorkspaceTile`), and the forwarded ports under the tree.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, Hsla, SharedString};
use serde_json::Value;

use super::chrome::ROW_RADIUS;
use super::menu::On;
use super::model::{Item, Section};
use super::{looks, model, tile, Asked, Sidebar};
use crate::ui::{ActivePalette, Crab, Status, StatusMark};

/// One row of the list.
#[derive(Clone, Debug, PartialEq)]
pub enum Drawn {
    Line(usize),
    Launch(usize),
    Creation(usize),
    Workspace(String),
    ForwardsHead,
    Forward(usize),
}

/// The words a creation's line takes for its step (`shell/creationLog.ts` CREATE_STEP_WORDS).
pub(crate) fn step_line(c: &super::Creation) -> String {
    let words = |stage: &str| match stage {
        "hostname-set" => "Naming the copy",
        "preview-route" => "Connecting to it",
        "daemon-answering" => "Waiting for it to answer",
        "project-cloned" => "Putting the project on it",
        "ready" => "Ready",
        "failed" => "Could not start",
        _ => "Making the copy",
    };
    let last = c.lines.iter().rev().find(|l| !c.failed || l.get("stage").and_then(Value::as_str) != Some("failed"));
    match last {
        Some(l) => l
            .get("waiting")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| words(l.get("stage").and_then(Value::as_str).unwrap_or_default()).to_owned()),
        None if c.failed => "Could not start".to_owned(),
        None => "Asking wsp to start it".to_owned(),
    }
}

impl Sidebar {
    /// The project a workspace belongs to, by its record.
    fn workspace_project(&self, workspace: &str) -> Option<String> {
        self.around.workspaces.get(workspace)?.pointer("/project/id")?.as_str().map(str::to_owned)
    }

    /// "project @ computer", the words a tile's first row says where it runs.
    fn where_words(&self, project: Option<&str>, workspace: Option<&str>) -> String {
        let project_name =
            project.and_then(|id| self.around.projects.iter().find(|(p, _)| p == id).map(|(_, n)| n.clone())).unwrap_or_default();
        let computer = workspace.and_then(|w| self.place_of(w)).and_then(|id| {
            self.around.places.iter().find(|p| p.get("id").and_then(Value::as_str) == Some(id.as_str())).map(looks::place_name)
        });
        // `workspaceComputerName`: a workspace no row places goes by its provider's name, else by where its kind is.
        let computer = computer.or_else(|| {
            let w = self.around.workspaces.get(workspace?)?;
            let field = |k: &str| w.get(k).and_then(Value::as_str);
            match field("kind") {
                Some("local") => None,
                kind => field("provider")
                    .map(looks::provider_key_name)
                    .or_else(|| (kind == Some("cloud")).then(|| "a provider".to_owned()))
                    .or_else(|| field("machineId").map(str::to_owned)),
            }
        });
        [project_name, computer.unwrap_or_default()].into_iter().filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" @ ")
    }

    fn in_pick(&self, project: Option<&str>) -> bool {
        self.prefs.project.as_deref().is_none_or(|p| project == Some(p))
    }

    /// WorkspaceSidebar.tsx madeIn: the workspaces being made under the pick, refused (Needs you's) and in flight (Working's).
    pub(super) fn creations_shown(&self) -> (Vec<usize>, Vec<usize>) {
        (0..self.around.creations.len())
            .filter(|&i| self.in_pick(self.around.creations[i].project.as_deref()))
            .partition(|&i| self.around.creations[i].failed)
    }

    /// Every row the list draws, top to bottom: the sends in flight, the threads' rows with the creations and the
    /// workspaces with no thread among them, then the forwarded ports.
    pub(crate) fn draw_rows(&self) -> Vec<Drawn> {
        let mut out: Vec<Drawn> = (0..self.around.launches.len())
            .filter(|&i| self.in_pick(self.workspace_project(&self.around.launches[i].workspace_id).as_deref()))
            .map(Drawn::Launch)
            .collect();
        let (failed, running) = self.creations_shown();
        let thread_workspaces: std::collections::HashSet<&str> = self.threads.iter().map(|t| t.workspace_id.as_str()).collect();
        let mut bare: Vec<(f64, String)> = self
            .around
            .workspaces
            .values()
            .filter(|w| w.get("kind").and_then(Value::as_str) == Some("cloud"))
            // A machine whose record names a listed thread as the one that started it is drawn under that thread.
            .filter(|w| !w.get("parentThreadId").and_then(Value::as_str).is_some_and(|lead| self.thread(lead).is_some()))
            .filter_map(|w| {
                let made = w.get("createdAt").and_then(Value::as_str).and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
                Some((made.map_or(0., |t| t.timestamp_millis() as f64), w.get("id")?.as_str()?.to_owned()))
            })
            .filter(|(_, id)| !thread_workspaces.contains(id.as_str()) && self.in_pick(self.workspace_project(id).as_deref()))
            .collect();
        bare.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut failed_done = failed.is_empty();
        for (ix, line) in self.lines.iter().enumerate() {
            if line.section == Section::Threads && line.depth == 0 {
                if let Item::Tile(t) = &line.item {
                    let started = self.threads[t.thread].started_at.unwrap_or(0.);
                    while bare.first().is_some_and(|(at, _)| *at >= started) {
                        out.push(Drawn::Workspace(bare.remove(0).1));
                    }
                }
            }
            out.push(Drawn::Line(ix));
            match line.item {
                Item::Head { section: Section::NeedsYou, folded, .. } if !failed_done => {
                    if !folded {
                        out.extend(failed.iter().map(|&i| Drawn::Creation(i)));
                    }
                    failed_done = true;
                }
                Item::Head { section: Section::Working, folded: false, .. } => out.extend(running.iter().map(|&i| Drawn::Creation(i))),
                _ => {}
            }
        }
        if !failed_done {
            let at = out.iter().position(|d| matches!(d, Drawn::Line(_))).unwrap_or(out.len());
            failed.iter().rev().for_each(|&i| out.insert(at, Drawn::Creation(i)));
        }
        out.extend(bare.into_iter().map(|(_, id)| Drawn::Workspace(id)));
        let forwards = self.forwards_shown().count();
        if forwards > 0 {
            out.push(Drawn::ForwardsHead);
            if !self.foot_folds.contains("forwards") {
                out.extend((0..forwards).map(Drawn::Forward));
            }
        }
        out
    }

    /// What a drawn row stands for across a rebuild.
    pub(crate) fn drawn_identity(&self, d: &Drawn) -> String {
        match d {
            Drawn::Line(ix) => self.lines.get(*ix).map(|l| self.identity(l)).unwrap_or_default(),
            Drawn::Launch(i) => format!("launch:{}", self.around.launches.get(*i).map_or("", |l| l.request_id.as_str())),
            Drawn::Creation(i) => format!("creation:{}", self.around.creations.get(*i).map_or("", |c| c.key.as_str())),
            Drawn::Workspace(id) => format!("ws:{id}"),
            Drawn::ForwardsHead => "forwards".to_owned(),
            Drawn::Forward(i) => format!("forward:{i}"),
        }
    }

    pub(crate) fn render_drawn(&mut self, ix: usize, window: &mut gpui_kit::Window, cx: &mut Context<Self>) -> AnyElement {
        match self.drawn_rows.get(ix).cloned() {
            Some(Drawn::Line(line)) => match self.lines.get(line).cloned() {
                Some(l) => tile::line(self, ix, &l, window, cx),
                None => div().into_any_element(),
            },
            Some(Drawn::Launch(i)) => self.launch_tile(i, cx),
            Some(Drawn::Creation(i)) => self.creation_tile(i, cx),
            Some(Drawn::Workspace(id)) => self.workspace_tile(&id, cx),
            Some(Drawn::ForwardsHead) => self.forwards_head(cx),
            Some(Drawn::Forward(i)) => self.forward_row(i, cx),
            None => div().into_any_element(),
        }
    }

    /// The two rows every tile draws, for a tile with no thread behind it.
    #[allow(clippy::too_many_arguments)]
    fn plain_tile(
        &self,
        id: SharedString,
        project: Option<&str>,
        place: String,
        slot: Option<AnyElement>,
        mark: Option<AnyElement>,
        title: String,
        title_ink: Hsla,
        crab: Option<AnyElement>,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let p = cx.palette().clone();
        let prefs = self.around.preferences.as_deref().unwrap_or(&super::NO_PREFERENCES);
        let row_one = h_flex()
            .h(px(14.))
            .min_w_0()
            .gap(px(6.))
            .text_size(px(11.))
            .line_height(px(14.))
            .text_color(p.sidebar_muted_foreground)
            .child(looks::project_glyph(project, prefs, &self.around.images, px(12.), p.sidebar_muted_foreground, true, &p))
            .child(div().flex_1().min_w_0().truncate().child(place))
            .children(slot);
        let row_two = h_flex()
            .h(px(18.))
            .min_w_0()
            .gap(px(6.))
            .children(mark)
            .child(
                div().flex_1().min_w_0().h(px(18.)).truncate().text_size(px(14.)).line_height(px(18.)).text_color(title_ink).child(title),
            )
            .children(crab);
        v_flex().id(id).h(tile::TILE).gap(px(4.)).p(px(8.)).rounded(ROW_RADIUS).child(row_one).child(row_two)
    }

    fn launch_tile(&mut self, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(l) = self.around.launches.get(i).cloned() else { return div().into_any_element() };
        let p = cx.palette().clone();
        let project = self.workspace_project(&l.workspace_id);
        let place = self.where_words(project.as_deref(), Some(&l.workspace_id));
        let ran = model::running_time(l.started.elapsed().as_secs_f64() * 1000.);
        let slot = StatusMark::new(Status::Working(ran.into())).crab(false).text(px(11.), px(14.)).into_any_element();
        let mark = tile::agent(&l.harness, cx);
        let crab = Crab::new(p.status_working).key(SharedString::from(format!("launch-crab-{}", l.request_id))).into_any_element();
        let tile = self.plain_tile(
            format!("launch-{}", l.request_id).into(),
            project.as_deref(),
            place,
            Some(slot),
            Some(mark),
            l.title,
            p.sidebar_foreground,
            Some(crab),
            cx,
        );
        div().px(px(8.)).child(tile).into_any_element()
    }

    fn creation_tile(&mut self, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(c) = self.around.creations.get(i).cloned() else { return div().into_any_element() };
        let p = cx.palette().clone();
        let place = self.where_words(c.project.as_deref(), c.workspace_id.as_deref());
        let slot = if c.failed {
            StatusMark::new(Status::Failed).text(px(11.), px(14.)).into_any_element()
        } else {
            StatusMark::new(Status::Starting)
                .text(px(11.), px(14.))
                .key(SharedString::from(format!("creation-crab-{}", c.key)))
                .into_any_element()
        };
        let hover = p.sidebar_row_hover;
        let hover_id = format!("c:{}", c.key);
        let card = crate::ui::hover_card::shows(cx.entity_id(), &hover_id, cx).then(|| {
            let opener = crate::ui::hover_card::opener(cx.entity_id(), hover_id.clone());
            div().absolute().inset_0().child(opener).child(self.creation_card(&c, place.clone(), cx))
        });
        let tile = self
            .plain_tile(
                format!("creation-{}", c.key).into(),
                c.project.as_deref(),
                place,
                Some(slot),
                None,
                c.name.clone(),
                p.sidebar_foreground,
                None,
                cx,
            )
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| this.hover_tile(&hover_id, *hovered, cx)))
            .when(c.failed, |el| {
                let key = c.key.clone();
                el.on_mouse_down(
                    gpui_kit::MouseButton::Right,
                    cx.listener(move |this, e: &gpui_kit::MouseDownEvent, window, cx| {
                        this.open_other_menu(On::Creation(key.clone()), e.position, window, cx)
                    }),
                )
            });
        div().px(px(8.)).child(div().relative().child(tile).children(card)).into_any_element()
    }

    /// A workspace's tile with no thread behind it: a press opens New thread on it.
    pub(super) fn workspace_card(&mut self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let Some(w) = self.around.workspaces.get(id).cloned() else { return div().into_any_element() };
        let p = cx.palette().clone();
        let project = self.workspace_project(id);
        let place = self.where_words(project.as_deref(), Some(id));
        let failed = model::start_failed(&w);
        let slot = failed.then(|| StatusMark::new(Status::Failed).text(px(11.), px(14.)).into_any_element());
        let name = w.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let hover = p.sidebar_row_hover;
        let (workspace, menu_on) = (id.to_owned(), id.to_owned());
        let renaming = self.renaming.as_ref().filter(|r| r.key == format!("w:{id}")).map(|r| r.input.clone());
        let name = if renaming.is_some() { String::new() } else { name };
        let tile = self
            .plain_tile(format!("ws-{id}").into(), project.as_deref(), place, slot, None, name, p.sidebar_muted_foreground, None, cx)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .on_click(cx.listener(move |_, _, _, cx| cx.emit(Asked::NewThreadIn(workspace.clone()))))
            .on_mouse_down(
                gpui_kit::MouseButton::Right,
                cx.listener(move |this, e: &gpui_kit::MouseDownEvent, window, cx| {
                    this.open_other_menu(On::Workspace(menu_on.clone()), e.position, window, cx)
                }),
            );
        let field = renaming.map(|input| self.rename_field(&input, cx).absolute().left(px(8.)).right(px(8.)).bottom(px(8.)).h(px(18.)));
        div().relative().child(tile).children(field).into_any_element()
    }

    /// A workspace with no thread at the root of the list, 8 px in as every row is.
    fn workspace_tile(&mut self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        div().px(px(8.)).child(self.workspace_card(id, cx)).into_any_element()
    }

    /// The cloud machines a listed thread started that hold no thread yet, newest first (threadTree.ts `forkedBy`).
    pub(super) fn machines(&self) -> Vec<model::Machine> {
        let held: std::collections::HashSet<&str> = self.threads.iter().map(|t| t.workspace_id.as_str()).collect();
        let mut out: Vec<model::Machine> = self
            .around
            .workspaces
            .values()
            .filter(|w| w.get("kind").and_then(Value::as_str) == Some("cloud"))
            .filter_map(|w| {
                let workspace = w.get("id")?.as_str()?.to_owned();
                let lead = w.get("parentThreadId")?.as_str()?;
                let made = w.get("createdAt").and_then(Value::as_str).and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
                (!held.contains(workspace.as_str()) && self.thread(lead).is_some()).then(|| model::Machine {
                    lead: lead.to_owned(),
                    workspace,
                    started_at: made.map_or(0., |t| t.timestamp_millis() as f64),
                })
            })
            .collect();
        out.sort_by(|a, b| b.started_at.total_cmp(&a.started_at).then_with(|| a.workspace.cmp(&b.workspace)));
        out
    }
}
