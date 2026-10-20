// SPDX-License-Identifier: AGPL-3.0-only
//! The card a tile opens to its right after the pointer rests on it 450 ms (`ThreadTile.tsx` TileCard,
//! `tileCard.ts`, `tileCheckout.ts`): the full title, the status line, then a line per fact with its glyph, then notes.

use std::time::{Duration, Instant};

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{anchored, deferred, div, point, prelude::*, px, relative, AnyElement, Context, FontWeight, Pixels};
use serde_json::{json, Value};
use wsp_host_client::protocol::WorkspacesCheckoutRequest;

use super::model::{self, Kind, Shape, Tile};
use super::{host, looks, tile, Sidebar};
use crate::ui::{Crab, Glyph, Palette};

/// `CARD_DELAY_MS`: a pass of the pointer down the list opens no card.
const DELAY: Duration = Duration::from_millis(450);
/// Base UI's `TooltipProvider` `timeout`: a card that closed this recently lets the next one open at once.
const NEXT_AT_ONCE: Duration = Duration::from_millis(400);
/// `max-w-72`.
const MAX_WIDTH: f32 = 288.;
/// `text-[13px]` sets no leading, so the card keeps the page's 1.5 (`html` in Tailwind's preflight).
const LEADING: f32 = 1.5;

/// `PULL_REQUEST_WORDS` in `pull-request.ts`.
fn pr_word(state: &str) -> &str {
    match state {
        "running" => "checks running",
        "failed" => "checks failed",
        "changesAsked" => "changes asked for",
        other => other,
    }
}

impl Sidebar {
    /// The pointer came onto a tile or left it, or the list scrolled one under it: a card opens once it has rested
    /// `DELAY` on one, and at once while another card stands or has just closed.
    pub(crate) fn hover_tile(&mut self, id: &str, hovered: bool, cx: &mut Context<Self>) {
        if !hovered {
            if self.hovered.as_deref() == Some(id) {
                self.hovered = None;
                self._card_wait = None;
                if self.card.take().is_some() {
                    self.card_closed = Some(Instant::now());
                    cx.notify();
                }
            }
            return;
        }
        self.hovered = Some(id.to_owned());
        let next = self.card.is_some() || self.card_closed.is_some_and(|at| at.elapsed() < NEXT_AT_ONCE);
        if next && self.renaming.is_none() && !self.menu.is_open() {
            self._card_wait = None;
            self.card = Some(id.to_owned());
            cx.notify();
            return;
        }
        let id = id.to_owned();
        self._card_wait = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DELAY).await;
            _ = this.update(cx, |this, cx| {
                if this.hovered.as_deref() == Some(id.as_str()) && this.renaming.is_none() && !this.menu.is_open() {
                    this.card = Some(id);
                    cx.notify();
                }
            });
        }));
    }

    /// Asks the host once a connection and phase for a workspace's checkout, which comes back on its pushed status: a
    /// workspace asked while paused is asked again once it wakes (`useCheckoutOf`).
    pub(crate) fn want_checkout(&mut self, workspace: &str, _: &mut Context<Self>) {
        let status = self.around.statuses.get(workspace).and_then(|s| s.get("phase")?.as_str());
        let phase = status.or_else(|| self.around.workspaces.get(workspace)?.get("phase")?.as_str()).unwrap_or_default();
        if self.around.checkouts_asked.get(workspace).is_some_and(|asked| asked == phase) {
            return;
        }
        self.around.checkouts_asked.insert(workspace.to_owned(), phase.to_owned());
        let Some(link) = self.link.clone() else { return };
        // Asked in passing: a refused read says nothing, and the next connection asks again.
        drop(link.runtime.spawn(host::write::<WorkspacesCheckoutRequest>(link.host.clone(), json!({ "workspaceId": workspace }))));
    }

    /// The card beside a tile `width` wide, in the page's palette.
    pub(crate) fn tile_card(&mut self, tile: &Tile, width: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let p = self.page.clone();
        let t = self.threads[tile.thread].clone();
        let prefs = self.around.preferences.clone().unwrap_or_default();
        let workspace = self.around.workspaces.get(&t.workspace_id).cloned().unwrap_or_default();
        let status = self.around.statuses.get(&t.workspace_id).cloned();
        let checkout = status.as_ref().and_then(|s| s.get("checkout").filter(|c| !c.is_null()).cloned());
        let glyph = |g: Glyph| Icon::new(g).with_size(px(12.)).into_any_element();
        let mut lines: Vec<(Option<AnyElement>, String)> = Vec::new();
        if !tile.started_by.is_empty() {
            let path: Vec<&str> = tile.started_by.iter().map(|&i| self.threads[i].title.as_str()).collect();
            lines.push((Some(glyph(Glyph::CornerDownRight)), format!("Started by {}", path.join(" / "))));
        }
        if let Some(forked) = &t.forked_from {
            lines.push((Some(glyph(Glyph::Split)), format!("Forked from {forked}")));
        }
        if let Some(name) = &t.project_name {
            let g = looks::project_glyph(t.project_id.as_deref(), &prefs, &self.around.images, px(12.), p.muted_foreground, true, &p);
            lines.push((Some(g), name.clone()));
        }
        if let Some(name) = self.computer_name(&t.workspace_id).filter(|n| !n.is_empty()) {
            let place = self.place_of(&t.workspace_id).and_then(|id| self.place(&id).cloned());
            lines.push((place.map(|place| looks::computer_glyph(&place, &prefs, px(12.), p.muted_foreground)), name));
        }
        let project_path = t.project_id.as_ref().and_then(|id| self.around.project_paths.get(id).cloned());
        if let Some(folder) = workspace.get("folder").and_then(Value::as_str).map(str::to_owned).or(project_path).filter(|f| !f.is_empty())
        {
            lines.push((Some(glyph(Glyph::Folder)), folder));
        }
        let branch = match &checkout {
            Some(c) => c.get("branch").and_then(Value::as_str).filter(|b| *b != "HEAD").unwrap_or_default().to_owned(),
            None => workspace.pointer("/worktree/branch").and_then(Value::as_str).unwrap_or_default().to_owned(),
        };
        if !branch.is_empty() {
            lines.push((Some(glyph(Glyph::GitBranch)), branch));
        }
        if !t.harness.is_empty() {
            // `WorkspaceSidebar.tsx`'s `model`: its catalog label (`modelOf` over `modelPicks`), the slug itself while no
            // catalog is read, the agent's own name (`agentName`) where the thread names no model.
            let catalog = self.around.listed.get(&t.workspace_id).and_then(|listed| {
                let row = listed.iter().find(|h| h.get("harness").and_then(Value::as_str) == Some(t.harness.as_str()))?;
                serde_json::from_value::<wsp_host_client::protocol::HarnessCatalog>(row.clone()).ok()
            });
            let name = match (&t.model, catalog) {
                (None, _) => crate::ui::catalog::name(&t.harness),
                (Some(model), None) => model.clone(),
                (Some(model), Some(catalog)) => {
                    use crate::screens::settings::pages::agents::model::{base_model, model_of};
                    model_of(&catalog, base_model(model)).label
                }
            };
            lines.push((Some(tile::agent(&t.harness, cx)), name));
        }
        if let Some(pr) = status.as_ref().and_then(|s| s.get("pr")).filter(|pr| pr.get("number").is_some()) {
            let n = pr.get("number").and_then(Value::as_u64).unwrap_or_default();
            lines.push((
                Some(glyph(Glyph::GitPullRequest)),
                format!("Pull request #{n}, {}", pr_word(pr.get("state").and_then(Value::as_str).unwrap_or("open"))),
            ));
        }
        if let Some(c) = &checkout {
            let changed = if c.get("editsUnread").and_then(Value::as_bool) == Some(true) {
                Some("changes not read".to_owned())
            } else {
                c.get("changed").and_then(Value::as_u64).filter(|n| *n > 0).map(|n| format!("{n} changed"))
            };
            if let Some(changed) = changed {
                lines.push((Some(glyph(Glyph::FileDiff)), changed));
            }
        }
        for note in [t.setup_refusal.clone(), tile.snoozed_working.map(|n| format!("Snoozed, {n} working in it"))].into_iter().flatten() {
            lines.push((None, note));
        }
        let settled = tile.shape == Shape::Settled || tile.snoozed_working.is_some();
        let status_line = (!settled).then(|| self.status_line(&t, tile, &p));
        // The status row's reason (`leadTree.ts` noteOf): what it asks, why it failed, or a finished turn's last line.
        let running = t.status == model::Status::Running;
        let reason = (!settled)
            .then(|| {
                t.asking_text
                    .clone()
                    .or_else(|| (t.status == model::Status::Failed).then(|| t.failure.clone()).flatten())
                    .or_else(|| (!running).then(|| t.last_line.clone()).flatten())
            })
            .flatten()
            .map(|r| div().debug_selector(|| "tile-card-reason".into()).pl(px(20.)).mt(px(2.)).line_clamp(3).child(r));
        let fact = |(g, text): (Option<AnyElement>, String)| {
            h_flex()
                .items_start()
                .gap(px(8.))
                .children(g.map(|g| div().debug_selector(|| format!("tile-card-glyph:{text}")).flex_none().mt(px(3.)).child(g)))
                .child(div().debug_selector(|| format!("tile-card-line:{text}")).min_w_0().child(text))
        };
        let card = div()
            .debug_selector(|| "tile-card".into())
            .max_w(px(MAX_WIDTH))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .bg(p.popover)
            .shadow(p.tooltip_shadow.clone())
            .text_size(px(13.))
            .line_height(relative(LEADING))
            .text_color(p.foreground)
            .child(
                v_flex()
                    .py(px(4.))
                    .gap(px(6.))
                    .child(
                        div()
                            .debug_selector(|| "tile-card-title".into())
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.foreground)
                            .child(t.title.clone()),
                    )
                    .child(
                        v_flex()
                            .gap(px(4.))
                            .text_color(p.muted_foreground)
                            .child(v_flex().children(status_line).children(reason))
                            .children(lines.into_iter().map(fact)),
                    ),
            );
        // `side="right"` with `sideOffset` 6, aligned to the tile's top.
        div()
            .absolute()
            .top_0()
            .left_0()
            .child(deferred(anchored().offset(point(width + px(6.), px(0.))).child(card)).with_priority(1))
            .into_any_element()
    }

    /// A creation's card: its name, its project and computer, the step it waits on.
    pub(crate) fn creation_card(&mut self, c: &super::Creation, place: String) -> AnyElement {
        let p = self.page.clone();
        let line = |text: String| div().min_w_0().child(text);
        let card =
            div()
                .max_w(px(MAX_WIDTH))
                .px(px(8.))
                .py(px(4.))
                .rounded(px(8.))
                .border_1()
                .border_color(p.border)
                .bg(p.popover)
                .text_size(px(13.))
                .line_height(relative(LEADING))
                .text_color(p.foreground)
                .child(
                    v_flex().py(px(4.)).gap(px(6.)).child(div().font_weight(FontWeight::MEDIUM).child(c.name.clone())).child(
                        v_flex().gap(px(4.)).text_color(p.muted_foreground).child(line(place)).child(line(super::rows::step_line(c))),
                    ),
                );
        div()
            .absolute()
            .top_0()
            .right_0()
            .child(deferred(anchored().offset(point(px(6.), px(0.))).child(card)).with_priority(1))
            .into_any_element()
    }

    /// The card's status row (`StatusLine.tsx`): the kind's glyph or the crab, its word, the time at the right.
    fn status_line(&self, t: &model::Thread, tile: &Tile, p: &Palette) -> AnyElement {
        let kind = model::tile_kind(t, tile, self.now);
        let slot = model::tile_slot(t, tile, self.now);
        let word = model::word(t, kind, self.now, super::utc_offset_ms() as f64);
        // `timed` on Working, `aged` on the kinds that end a turn; the rest say no time.
        let time = match kind {
            Kind::Working => Some(model::running_time(self.now - t.started_at.unwrap_or(self.now))),
            Kind::Resting | Kind::Finished | Kind::Done | Kind::Failed | Kind::Expired | Kind::Released | Kind::Stopped => {
                tile.age_at.map(|at| model::resting_age(at, self.now))
            }
            _ => None,
        };
        status_row(kind, slot.glyph, slot.tone, format!("card-crab-{}", t.key), word.to_string(), time, p)
    }
}

/// A card's status row (`StatusLine.tsx`): the kind's glyph or the crab, its word in the foreground, or medium in the
/// kind's tone, then the time at the right. A resting thread says how it ended, as Finished's muted check and word.
pub(crate) fn status_row(
    kind: Kind,
    glyph: Option<&str>,
    tone: Option<model::Tone>,
    crab_key: String,
    word: String,
    time: Option<String>,
    p: &Palette,
) -> AnyElement {
    let (kind, glyph, tone, word) = match kind {
        Kind::Resting => (Kind::Finished, model::kind_slot(Kind::Finished).0, None, model::kind_word(Kind::Finished).to_owned()),
        _ => (kind, glyph, tone, word),
    };
    let tone = match tone {
        Some(model::Tone::Input) => Some(p.status_input),
        Some(model::Tone::Working) => Some(p.status_working),
        Some(model::Tone::Failed) => Some(p.status_failed),
        Some(model::Tone::Done) => Some(p.status_done),
        Some(model::Tone::Warning) => Some(p.warning_foreground),
        None => None,
    };
    let ink = tone.unwrap_or(p.muted_foreground);
    let mark: Option<AnyElement> = if kind == Kind::Working || kind == Kind::Starting {
        Some(Crab::new(p.status_working).key(gpui_kit::SharedString::from(crab_key)).into_any_element())
    } else {
        glyph.map(|g| {
            let glyph = match g {
                "message-circle-question" => Glyph::MessageCircleQuestionMark,
                "hourglass" => Glyph::Hourglass,
                "gauge" => Glyph::Gauge,
                "circle-alert" => Glyph::CircleAlert,
                "cloud-off" => Glyph::CloudOff,
                "calendar-x-2" => Glyph::CalendarX2,
                "circle-stop" => Glyph::CircleStop,
                "upload" => Glyph::Upload,
                _ => Glyph::CircleCheck,
            };
            Icon::new(glyph).with_size(px(12.)).text_color(ink).into_any_element()
        })
    };
    h_flex()
        .gap(px(8.))
        .child(div().debug_selector(|| "card-status-mark".into()).flex_none().w(px(12.)).flex().justify_center().children(mark))
        .child(
            div()
                .debug_selector(|| format!("card-status:{word}"))
                .flex_none()
                .when_some(tone, |el, ink| el.font_weight(FontWeight::MEDIUM).text_color(ink))
                .when(tone.is_none(), |el| el.text_color(p.foreground))
                .child(word),
        )
        .child(div().flex_1())
        .children(time.map(|t| div().debug_selector(|| "card-status-time".into()).flex_none().child(t)))
        .into_any_element()
}
