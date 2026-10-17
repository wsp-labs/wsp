// SPDX-License-Identifier: AGPL-3.0-only
//! The composer alone in a window, as `desktop-gpui --screen composer` opens it for captures: at the foot of a thread
//! column on the page's ground, tied to the host serving the state file, on the thread `--open` names or else the
//! newest.

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::v_flex;
use gpui_kit::{prelude::*, Context, Entity, Task, Window};
use tokio::runtime::Runtime;
use wsp_host_client::{Env, Failure, HostClient};

use super::link::{Link, Target};
use super::model::Hold;
use super::Composer;
use crate::ui::ActivePalette;

pub struct ComposerScreen {
    composer: Entity<Composer>,
    link: Option<Entity<Link>>,
    note: Option<String>,
    _connect: Task<()>,
}

impl ComposerScreen {
    pub fn new(state: PathBuf, env: Env, runtime: Arc<Runtime>, open: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| {
            let mut composer = Composer::new(window, cx);
            composer.set_hold(Some(Hold::Connecting), cx);
            composer
        });
        let reading = runtime.spawn({
            let runtime = runtime.clone();
            async move {
                let host = Arc::new(HostClient::connect(&state, &env).await?);
                let threads = host.threads().await?;
                let row = match &open {
                    Some(id) => threads.into_iter().find(|t| t.thread_id.as_deref() == Some(id.as_str()) || &t.id == id),
                    None => threads.into_iter().max_by(|a, b| a.started_at.unwrap_or(0.).total_cmp(&b.started_at.unwrap_or(0.))),
                };
                let row = row.ok_or_else(|| Failure::new("the host lists no such thread"))?;
                let target = Target { workspace_id: row.workspace_id.clone(), thread_id: row.thread_id.clone() };
                let opened = Link::read(&host, &runtime, &target).await?;
                Ok::<_, Failure>((host, target, opened))
            }
        });
        let connect = cx.spawn_in(window, async move |this, cx| {
            let read = reading.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
            _ = this.update_in(cx, |this, window, cx| match read {
                Ok((host, target, opened)) => {
                    let composer = this.composer.clone();
                    this.link = Some(cx.new(|cx| Link::new(host, runtime, target, opened, composer, window, cx)));
                    cx.notify();
                }
                Err(failure) => {
                    eprintln!("desktop-gpui: {}", failure.message);
                    this.composer.update(cx, |c, cx| c.set_hold(Some(Hold::HostDown), cx));
                    this.note = Some(failure.message);
                    cx.notify();
                }
            });
        });
        ComposerScreen { composer, link: None, note: None, _connect: connect }
    }

    pub fn composer(&self) -> &Entity<Composer> {
        &self.composer
    }
}

/// The thread column the composer sits in, as ChatView.tsx docks it: at the foot, or on a thread with no message
/// yet centred in the view under the thread's 52 px header, over the 88 px its heading takes.
pub fn column(fresh: bool, ground: gpui_kit::Hsla, composer: impl IntoElement) -> gpui_kit::Div {
    let column = v_flex().size_full().bg(ground);
    if fresh {
        column
            .child(gpui_kit::div().h(gpui_kit::px(52.)))
            .child(v_flex().flex_1().justify_center().child(gpui_kit::div().h(gpui_kit::px(88.))).child(composer))
    } else {
        column.justify_end().child(composer)
    }
}

impl Render for ComposerScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette();
        let fresh = self.composer.read(cx).fresh();
        column(fresh, p.background, self.composer.clone())
            .text_color(p.foreground)
            .children(self.note.clone().map(|note| gpui_kit::div().p_4().text_sm().text_color(p.muted_foreground).child(note)))
    }
}
