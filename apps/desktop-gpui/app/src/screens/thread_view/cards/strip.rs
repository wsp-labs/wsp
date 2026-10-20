// SPDX-License-Identifier: AGPL-3.0-only
//! `SubStrip`: the one row over a sub-thread's composer while its machine holds or blocks it.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, App, SharedString};

use super::spawn::go;
use super::threads::on_sidebar;
#[cfg(test)]
use super::threads::RAN;
use crate::screens::Route;
use crate::thread_view::ThreadView;
use crate::transcript;
use crate::ui::{ActivePalette, Button, Glyph};

/// `SubStrip`: the one row over a sub-thread's composer while its machine holds or blocks it, in the composer's own
/// box: the state's glyph in its frame, the state as the title with the why under it, and its acts.
pub fn strip(view: &gpui_kit::Entity<ThreadView>, strip: &transcript::Strip, cx: &App) -> AnyElement {
    let p = cx.palette();
    let workspace = view.read(cx).followed_workspace().or(view.read(cx).listing().and_then(|l| l.workspace.as_deref())).map(str::to_owned);
    let glyph = match strip.key {
        "waiting-minute" | "waiting-machines" => Glyph::Hourglass,
        "oom" => Glyph::MemoryStick,
        "released" => Glyph::CloudOff,
        "not-pushed" => Glyph::Upload,
        "forgotten" => Glyph::Unplug,
        _ => Glyph::Timer,
    };
    let words = h_flex().min_w_0().gap(px(12.)).child(crate::ui::glyph_frame(glyph, cx)).child(
        v_flex()
            .min_w_0()
            .gap(px(2.))
            .child(
                div()
                    .debug_selector(|| "sub-strip-title".into())
                    .text_size(px(15.))
                    .line_height(px(24.))
                    .text_color(p.foreground)
                    .child(strip.title.clone()),
            )
            .child(
                div().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).whitespace_normal().child(strip.note.clone()),
            ),
    );
    let acts: Vec<AnyElement> = strip
        .acts
        .iter()
        .enumerate()
        .map(|(i, act)| {
            let id = format!("sub-strip-act-{i}");
            let button = Button::outline(SharedString::from(id.clone()), act.word()).xs();
            let button = match act {
                transcript::StripAct::Raise(place) => {
                    let at = place.as_ref().map_or_else(|| "settings:computers".to_owned(), |p| format!("settings:computer:{p}"));
                    button.on_click(move |_, _, cx| go(Route::Screen(at.clone().into()), cx))
                }
                // The shell opens both dialogs on its own host, as the sidebar's own acts ask it to.
                transcript::StripAct::CopyHome(_) => {
                    let workspace = workspace.clone();
                    button.on_click(move |_, _, cx| {
                        let Some(workspace) = workspace.clone() else { return };
                        on_sidebar(format!("export {workspace}"), cx, |_, cx| cx.emit(crate::sidebar::Asked::ExportProject(workspace)));
                    })
                }
                transcript::StripAct::AddAgain(_) => button.on_click(move |_, _, cx| {
                    go(Route::Screen("settings:computers".into()), cx);
                    on_sidebar("add a cloud".to_owned(), cx, |_, cx| cx.emit(crate::sidebar::Asked::AddCloud));
                }),
                transcript::StripAct::StartAgain { lead, harness, prompt, .. } => {
                    let params = serde_json::json!({
                        "workspaceId": lead.workspace, "thread": lead.thread, "harness": harness, "prompt": prompt,
                    });
                    let said = format!("send {} {} {harness} {prompt}", lead.workspace, lead.thread);
                    button.on_click(move |_, _, cx| {
                        let mut params = params.clone();
                        params["requestId"] = crate::screens::composer::link::request_id().into();
                        on_sidebar(said.clone(), cx, |s, cx| s.write_op::<wsp_host_client::protocol::SessionsStartRequest>(params, cx));
                    })
                }
                transcript::StripAct::PushAgain(branch) => {
                    let (view, prompt) = (view.downgrade(), transcript::push_again_prompt(branch));
                    button.on_click(move |_, _, cx| {
                        #[cfg(test)]
                        RAN.with(|r| r.borrow_mut().push(format!("send {prompt}")));
                        _ = view.update(cx, |v, cx| v.send(prompt.clone(), cx));
                    })
                }
            };
            div().debug_selector(move || id.clone()).child(button).into_any_element()
        })
        .collect();
    let two = acts.len() > 1;
    let row = match acts.len() {
        0 => h_flex().min_h(px(60.)).px(px(20.)).py(px(12.)).child(words),
        _ if two => v_flex()
            .min_h(px(60.))
            .px(px(20.))
            .py(px(12.))
            .gap(px(12.))
            .child(words)
            .child(h_flex().flex_wrap().gap(px(8.)).pl(px(44.)).children(acts)),
        _ => h_flex()
            .min_h(px(60.))
            .px(px(20.))
            .py(px(12.))
            .justify_between()
            .gap(px(20.))
            .child(words)
            .child(h_flex().flex_none().children(acts)),
    };
    div()
        .debug_selector(move || format!("sub-strip-{}", strip.key))
        .w_full()
        .px(px(20.))
        .child(div().mx_auto().w_full().max_w(px(768.)).min_w_0().child(crate::screens::composer::parts::glass_box(cx).child(row)))
        .into_any_element()
}
