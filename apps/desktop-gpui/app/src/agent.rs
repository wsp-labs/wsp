// SPDX-License-Identifier: AGPL-3.0-only
//! gpui-agent's hold on the app, under the `agent` feature, which no release turns on: it listens only when the app was
//! started with `GPUI_AGENT_SOCKET`, and its probes answer with what screens check most, the thread on screen and the
//! composer's text.

use gpui_kit::component::Root;
use gpui_kit::{App, Entity};
use serde_json::{json, Value};

use crate::screens::composer::screen::ComposerScreen;
use crate::screens::composer::Composer;
use crate::screens::Route;
use crate::shell::Shell;

pub fn init(cx: &mut App) {
    gpui_agent::init(cx);
    gpui_agent::probe(cx, "thread", |cx| match shell(cx).map(|s| s.read(cx).route().clone()) {
        Some(Route::Thread { workspace_id, thread_id }) => json!({ "workspace": workspace_id, "thread": thread_id }),
        _ => Value::Null,
    });
    gpui_agent::probe(cx, "composer", |cx| composer(cx).map_or(Value::Null, |c| c.read(cx).text(cx).into()));
}

fn shell(cx: &App) -> Option<Entity<Shell>> {
    cx.windows().into_iter().find_map(|w| w.downcast::<Root>()?.read(cx).ok()?.view().clone().downcast::<Shell>().ok())
}

/// The composer on screen: the open thread's, else a New thread page's.
fn composer(cx: &App) -> Option<Entity<Composer>> {
    let shell = shell(cx)?.read(cx);
    if matches!(shell.route(), Route::Thread { .. }) {
        return Some(shell.thread_screen()?.read(cx).composer().clone());
    }
    Some(shell.screen_view()?.downcast::<ComposerScreen>().ok()?.read(cx).composer().clone())
}
