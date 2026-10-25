// SPDX-License-Identifier: AGPL-3.0-only
//! The whole chat takes a dropped file, not the composer's box alone, and says so while a file is over it
//! (`ChatDropZone.tsx`): the settings card drawn large over the chat, with the attach glyph, what a drop does and the
//! caps a file meets. The chat is the thread's screen under the header, or the New thread page.

use gpui_kit::component::v_flex;
use gpui_kit::{div, prelude::*, px, App, Context, Div, DragMoveEvent, Entity, ExternalPaths, FontWeight, Window};

use super::model::{self, Hold};
use super::{parts, Composer};
use crate::ui::{glyph_frame, size, text, theme::WspTheme, ActivePalette, Glyph};

/// `DROP_WORDS.title`.
pub const DROP_TITLE: &str = "Drop files to attach";

/// `DROP_WORDS.caps`, from the caps attachments.ts sets.
pub fn drop_caps() -> String {
    let mb = model::FILE_MAX_BYTES >> 20;
    format!("At most {}: an image (PNG, JPEG, GIF or WebP) up to {mb} MB, any other file up to {mb} MB.", model::FILES_MAX)
}

/// `chat` taking files dropped anywhere on it into `composer`, with the frame drawn over it while a file is over it
/// and the composer takes files. The chat's screen draws cached, so the screen is told when the frame comes and goes.
pub fn chat<V: 'static>(chat: Div, composer: &Entity<Composer>, window: &Window, cx: &mut Context<V>) -> Div {
    if composer.read(cx).dragging && !cx.has_active_drag() {
        composer.update(cx, |c, _| c.dragging = false);
    }
    let c = composer.read(cx);
    let shown = c.dragging && !c.hold.as_ref().is_some_and(Hold::shuts);
    let moved = composer.clone();
    // A drag's moves reach every listener in the window, inside the chat or not.
    chat.on_drag_move::<ExternalPaths>(cx.listener(move |_, e: &DragMoveEvent<ExternalPaths>, _, cx| {
        let over = e.bounds.contains(&e.event.position);
        if moved.read(cx).dragging != over {
            moved.update(cx, |c, _| c.dragging = over);
            cx.notify();
        }
    }))
    .on_drop(take(composer, cx))
    // A drop reaches only the topmost hitbox, so the frame over every card takes it once it stands.
    .when(shown, |el| el.child(overlay(crate::ui::narrow(window), cx).id("chat-drop").occlude().on_drop(take(composer, cx))))
}

/// Attaches the dropped files unless the composer is shut, and takes the frame down.
fn take<V: 'static>(composer: &Entity<Composer>, cx: &mut Context<V>) -> impl Fn(&ExternalPaths, &mut Window, &mut App) + 'static {
    let (composer, screen) = (composer.clone(), cx.entity().downgrade());
    move |paths: &ExternalPaths, _: &mut Window, cx: &mut App| {
        composer.update(cx, |c, cx| {
            c.dragging = false;
            if !c.hold.as_ref().is_some_and(Hold::shuts) {
                c.attach_paths(paths.paths(), cx);
            }
        });
        _ = screen.update(cx, |_, cx| cx.notify());
    }
}

/// `ChatDropFrame`: the chat's ground at 80%, blurred off the Mac, and a card saying what a drop takes.
pub fn overlay(narrow: bool, cx: &App) -> Div {
    let p = cx.palette();
    let g = parts::glass(cx);
    let mac = cx.global::<WspTheme>().look.mac;
    let (title, note) = (text::SETTING_TITLE, text::NOTE);
    div()
        .debug_selector(|| "chat-drop".into())
        .absolute()
        .inset_0()
        .flex()
        .map(|el| if narrow { el.p_3() } else { el.p_5() })
        .bg(p.background.opacity(0.8))
        .when(!mac && crate::glass::composer_blurs(), |el| el.backdrop_blur(g.blur).backdrop_saturation(g.saturation))
        .child(
            v_flex()
                .debug_selector(|| "chat-drop-card".into())
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .px_6()
                .rounded(size::CARD_RADIUS)
                .border_1()
                .border_color(p.border.opacity(0.6))
                .bg(p.foreground.opacity(0.03))
                .child(glyph_frame(Glyph::Paperclip, cx))
                .child(
                    v_flex()
                        .max_w(px(320.))
                        .gap_1()
                        .items_center()
                        .text_center()
                        .child(
                            div()
                                .text_size(title.0)
                                .line_height(title.1)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.foreground)
                                .child(DROP_TITLE),
                        )
                        .child(div().text_size(note.0).line_height(note.1).text_color(p.muted_foreground).child(drop_caps())),
                ),
        )
}
