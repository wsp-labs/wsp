// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's drawn parts: the glass box, the pickers' triggers, the attach, send and stop buttons, the files row,
//! the tray under the box, the banner and the queue's drawer over it, a mention chip, and the drop overlay. Sizes are
//! ComposerSurface.tsx's, ComposerOptionPickers.tsx's, ComposerPrimaryActions.tsx's and ComposerCheckoutRow.tsx's.

use gpui_kit::component::input::InlineTokenContext;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    div, img, point, prelude::*, px, svg, AnyElement, App, BoxShadow, Context, Div, FontWeight, Hsla, Pixels, SharedString, Stateful,
};

use super::model::{self, Layout};
use super::{menus, Banner, Composer, Popup};
use crate::ui::icons::{SEND, STOP};
use crate::ui::{catalog, ActivePalette, Button, Glyph, Palette};

const RADIUS: Pixels = px(22.);
/// `--chat-composer-drawer-inset`: how far the tray, the drawer, the banner and the menus sit in from the box's sides.
pub const INSET: Pixels = px(22.);
/// What shows of the tray under the box.
pub const TRAY: Pixels = px(32.);

/// The glass (index.css and ComposerSurface.tsx): the tint, the hairline and the dark side's top light. The light side's
/// drop shadow, the theme's `composer_shadow`, is not drawn: GPUI skips a shadow whose own rect lies outside its clip,
/// and unclipped it shows through the glass.
pub struct Glass {
    pub tint: Hsla,
    pub outline: Hsla,
    pub highlight: Hsla,
    pub blur: Pixels,
    pub saturation: f32,
}

impl Glass {
    /// The blur and its saturation over what lies under `el`, unless the composer's glass is turned off.
    pub fn frost<E: Styled>(&self, el: E) -> E {
        if crate::glass::composer_blurs() {
            el.backdrop_blur(self.blur).backdrop_saturation(self.saturation)
        } else {
            el
        }
    }
}

pub fn glass(cx: &App) -> Glass {
    let p = cx.palette();
    if cx.theme().is_dark() {
        Glass {
            tint: p.foreground.opacity(0.03),
            outline: p.composer_outline,
            highlight: p.composer_highlight,
            blur: px(16.),
            saturation: 1.08,
        }
    } else {
        Glass { tint: p.card.opacity(0.55), outline: p.composer_outline, highlight: p.composer_highlight, blur: px(12.), saturation: 1.14 }
    }
}

/// The box: radius 22, its hairline standing in for the web's 1 px padding under its outline, the frosted tint and the
/// dark side's top light.
pub fn glass_box(cx: &App) -> Div {
    let g = glass(cx);
    let shadows =
        vec![BoxShadow { color: g.highlight, offset: point(px(0.), px(1.)), blur_radius: px(0.), spread_radius: px(0.), inset: true }];
    g.frost(
        div().debug_selector(|| "composer-box".into()).relative().w_full().rounded(RADIUS).border_1().border_color(g.outline).bg(g.tint),
    )
    .shadow(shadows)
}

/// A mention chip in the text: 17 px high in the chip's 12 px type, a 6 px radius, the border at 70% and the accent at
/// 40%, the file's glyph at 85%.
pub fn chip(ctx: &InlineTokenContext, cx: &App) -> AnyElement {
    let p = cx.palette();
    let id = ctx.token().id().to_string();
    let glyph = match id.split_once(':').map_or("", |(kind, _)| kind) {
        "skill" => Glyph::Package,
        "pr" => Glyph::GitPullRequest,
        "issue" => Glyph::CircleDot,
        _ if id.ends_with('/') => Glyph::Folder,
        _ => Glyph::File,
    };
    h_flex()
        .id("composer-chip")
        .h(px(17.))
        .max_w(ctx.available_width())
        .gap(px(4.))
        .px(px(6.))
        .rounded(px(6.))
        .border_1()
        .border_color(p.border.opacity(0.7))
        .bg(if ctx.is_selected() { p.ring.opacity(0.3) } else { p.accent.opacity(0.4) })
        .text_size(px(12.04))
        .line_height(px(12.04))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.foreground)
        .child(Icon::new(glyph).with_size(px(14.)).text_color(p.muted_foreground).opacity(0.85))
        .child(div().min_w_0().truncate().child(ctx.token().label().clone()))
        .into_any_element()
}

/// One picker's trigger: a ghost button, 32 px and 15 px in the full layout, 28 px and 13 px on one line.
fn trigger(id: &'static str, open: bool, compact: bool, p: &Palette) -> Stateful<Div> {
    let (h, gap, size) = if compact { (px(28.), px(6.), px(13.)) } else { (px(32.), px(8.), px(15.)) };
    h_flex()
        .id(id)
        .debug_selector(move || id.into())
        .flex_none()
        .h(h)
        .px_2()
        .gap(gap)
        .rounded(px(8.))
        .border_1()
        .border_color(gpui_kit::transparent_black())
        .text_size(size)
        .line_height(px(20.))
        .text_color(if open { p.foreground } else { p.muted_foreground })
        .when(open, |el| el.bg(p.accent))
        .cursor_pointer()
        .hover(|s| s.bg(p.accent).text_color(p.foreground))
}

fn chevron(p: &Palette) -> impl IntoElement {
    Icon::new(Glyph::ChevronDown).with_size(px(14.)).text_color(p.muted_foreground.opacity(0.6))
}

/// The access mode's lucide glyph (ComposerOptionPickers.tsx `accessIcon`).
pub fn access_glyph(value: &str) -> Glyph {
    match value {
        "default" | "read-only" => Glyph::Lock,
        "acceptEdits" | "workspace-write" | "auto_edit" => Glyph::PenLine,
        "plan" => Glyph::PencilRuler,
        "bypassPermissions" | "danger-full-access" | "yolo" => Glyph::LockOpen,
        "auto" => Glyph::Sparkles,
        "manual" => Glyph::Hand,
        "dontAsk" => Glyph::CircleSlash,
        _ => Glyph::Shield,
    }
}

/// Model, then reasoning, then (in the full layout, where the box is wider than 640 px) access, each a trigger that
/// opens its popup.
pub fn pickers(c: &Composer, layout: Layout, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let r = c.resolved();
    let compact = layout != Layout::Full;
    let rule = || div().mx(px(6.)).h(px(20.)).w(px(1.)).flex_none().bg(p.border);
    // On one line the web's triggers stand edge to edge; the full layout keeps 4 px either side of its rules.
    let mut row = h_flex().debug_selector(|| "composer-pickers".into()).flex_none().items_center().when(!compact, |el| el.gap_1());
    let Some(agent) = r.agent.clone() else { return row.into_any_element() };
    row = row.child(
        div()
            .relative()
            .child(
                trigger("picker-model", c.popup == Some(Popup::Model), compact, &p)
                    .child(catalog::mark(&agent.harness, &agent.label, px(16.), p.foreground, cx))
                    .child(div().truncate().child(r.model_word()))
                    .child(chevron(&p))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_popup(Popup::Model, cx))),
            )
            .children(menus::picker(c, Popup::Model, cx)),
    );
    let fast_offered = r.model.as_ref().is_some_and(|m| m.fast == Some(true));
    if !r.efforts.is_empty() || !r.windows.is_empty() || fast_offered {
        let only_fast = r.efforts.is_empty() && r.windows.is_empty();
        let word = match (only_fast, c.fast()) {
            (true, true) => "Fast".to_owned(),
            (true, false) => "Normal".to_owned(),
            (false, _) => r.reasoning_word(),
        };
        row = row.when(!compact, |el| el.child(rule())).child(
            div()
                .relative()
                .child(
                    trigger("picker-reasoning", c.popup == Some(Popup::Reasoning), compact, &p)
                        .when(!only_fast, |el| el.child(Icon::new(Glyph::Brain).with_size(px(16.))))
                        .child(word)
                        .when(fast_offered && c.fast(), |el| el.child(Icon::new(Glyph::Zap).with_size(px(14.)).opacity(0.8)))
                        .child(chevron(&p))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_popup(Popup::Reasoning, cx))),
                )
                .children(menus::picker(c, Popup::Reasoning, cx)),
        );
    }
    if !compact && !agent.permission_modes.is_empty() {
        let glyph = access_glyph(r.access.as_ref().map_or("", |a| a.value.as_str()));
        row = row.child(rule()).child(
            div()
                .relative()
                .child(
                    trigger("picker-access", c.popup == Some(Popup::Access), compact, &p)
                        .child(Icon::new(glyph).with_size(px(16.)))
                        .child(r.access_word())
                        .child(chevron(&p))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_popup(Popup::Access, cx))),
                )
                .children(menus::picker(c, Popup::Access, cx)),
        );
    }
    row.into_any_element()
}

/// Attach, then send or stop.
pub fn actions(c: &Composer, shut: bool, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let attach = h_flex()
        .id("composer-attach")
        .debug_selector(|| "composer-attach".into())
        .flex_none()
        .justify_center()
        .size(px(28.))
        .rounded(px(8.))
        .text_color(p.muted_foreground)
        .child(Icon::new(Glyph::Paperclip).with_size(px(16.)))
        .map(|el| {
            if shut {
                el.opacity(0.64)
            } else {
                el.cursor_pointer()
                    .hover(|s| s.bg(p.accent).text_color(p.foreground))
                    .on_click(cx.listener(|this, _, window, cx| this.pick_files(window, cx)))
            }
        });
    // `shadow-xs`, the controls' own shadow.
    let lift = p.control_shadow.clone();
    let light = |color: Hsla| {
        div().absolute().inset_0().rounded_full().shadow(vec![BoxShadow {
            color,
            offset: point(px(0.), px(1.)),
            blur_radius: px(0.),
            spread_radius: px(0.),
            inset: true,
        }])
    };
    let circle = |id: &'static str| {
        h_flex().id(id).debug_selector(move || id.into()).relative().flex_none().justify_center().size(px(32.)).rounded_full()
    };
    let main = if c.running {
        let stopping = c.stopping;
        circle("composer-stop")
            .bg(p.destructive.opacity(0.9))
            .shadow(vec![BoxShadow { color: p.destructive.opacity(0.24), ..lift[0].clone() }])
            .child(light(p.button_light))
            .text_color(p.destructive_ink)
            .map(|el| {
                if stopping {
                    el.child(Icon::new(Glyph::LoaderCircle).with_size(px(14.)))
                } else {
                    el.child(svg().path(STOP).size(px(12.)).text_color(p.destructive_ink))
                        .cursor_pointer()
                        .hover(|s| s.bg(p.destructive))
                        .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
                }
            })
            .into_any_element()
    } else {
        let empty = c.editor.read(cx).value().trim().is_empty();
        let in_flight = c.hold == Some(model::Hold::InFlight);
        let arrow = |ink: Hsla| svg().path(SEND).size(px(14.)).text_color(ink);
        // The web fades the empty send as one layer, its arrow keeping the ground's color; GPUI fades each part on its
        // own, so the faded fill is drawn solid and the arrow is left out of the fade.
        let fill = if empty { p.background.blend(p.foreground.opacity(0.3)) } else { p.foreground };
        match c.send_held(cx) {
            Some(words) => {
                let words = SharedString::from(words);
                circle("composer-send")
                    .border_1()
                    .border_color(p.input)
                    .bg(p.input_fill)
                    .child(div().absolute().inset_0().rounded_full().shadow(vec![BoxShadow {
                        color: p.keycap_top,
                        offset: point(px(0.), px(1.)),
                        blur_radius: px(0.),
                        spread_radius: px(0.),
                        inset: true,
                    }]))
                    .map(|el| {
                        if in_flight {
                            el.child(Icon::new(Glyph::LoaderCircle).with_size(px(14.)).text_color(p.muted_foreground))
                        } else {
                            el.child(arrow(p.muted_foreground))
                        }
                    })
                    .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(words.clone()).build(window, cx))
                    .into_any_element()
            }
            None if c.banner.as_ref().is_some_and(|b| b.resume.is_some()) => h_flex()
                .id("composer-send")
                .debug_selector(|| "composer-send".into())
                .relative()
                .flex_none()
                .h(px(32.))
                .px(px(14.))
                .rounded_full()
                .bg(fill)
                .when(!empty, |el| el.shadow(lift.clone()).child(light(p.button_light)))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.background)
                .child("Continue")
                .map(|el| {
                    if empty {
                        el
                    } else {
                        el.cursor_pointer()
                            .hover(|s| s.bg(p.foreground.opacity(0.9)))
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))
                    }
                })
                .into_any_element(),
            None => circle("composer-send")
                .bg(fill)
                .when(!empty, |el| el.shadow(lift.clone()).child(light(p.button_light)))
                .child(arrow(p.background))
                .map(|el| {
                    if empty {
                        el
                    } else {
                        el.cursor_pointer()
                            .hover(|s| s.bg(p.foreground.opacity(0.9)))
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))
                    }
                })
                .into_any_element(),
        }
    };
    h_flex().debug_selector(|| "composer-actions".into()).flex_none().items_center().gap_2().child(attach).child(main).into_any_element()
}

/// The files waiting to go: 56 px thumbnails and tiles, each with its remove button.
pub fn files_row(c: &Composer, cx: &mut Context<Composer>) -> Option<AnyElement> {
    if c.files.is_empty() && c.refused.is_empty() {
        return None;
    }
    let p = cx.palette().clone();
    let remove = |ix: usize, cx: &mut Context<Composer>| {
        h_flex()
            .id(("composer-file-remove", ix))
            .absolute()
            .top(px(-6.))
            .right(px(-6.))
            .size(px(20.))
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(p.border.opacity(0.6))
            .bg(p.card)
            .text_color(p.muted_foreground)
            .opacity(0.)
            .group_hover("composer-file", |s| s.opacity(1.))
            .cursor_pointer()
            .child(Icon::new(Glyph::X).with_size(px(14.)))
            .on_click(cx.listener(move |this, _, _, cx| this.remove_file(ix, cx)))
    };
    let tile = || h_flex().group("composer-file").relative().h(px(56.)).rounded(px(10.)).border_1().border_color(p.border.opacity(0.6));
    let mut row = h_flex().debug_selector(|| "composer-files".into()).flex_wrap().gap(px(6.)).px_4().pt_3();
    for (ix, file) in c.files.iter().enumerate() {
        let shown = if file.is_image() {
            let image = std::sync::Arc::new(gpui_kit::Image::from_bytes(
                gpui_kit::ImageFormat::from_mime_type(&file.media_type).unwrap_or(gpui_kit::ImageFormat::Png),
                file.bytes.to_vec(),
            ));
            tile().w(px(56.)).bg(p.card.opacity(0.5)).child(img(image).size_full().rounded(px(9.)).object_fit(gpui_kit::ObjectFit::Cover))
        } else {
            tile()
                .max_w(px(192.))
                .gap_2()
                .pl(px(10.))
                .pr_3()
                .bg(p.card.opacity(0.5))
                .child(Icon::new(Glyph::FileText).with_size(px(16.)).text_color(p.muted_foreground))
                .child(
                    v_flex()
                        .min_w_0()
                        .child(div().truncate().text_size(px(13.)).line_height(px(20.)).text_color(p.foreground).child(file.name.clone()))
                        .child(
                            div()
                                .text_size(px(11.))
                                .line_height(px(16.))
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_color(p.muted_foreground)
                                .child(file.size_word()),
                        ),
                )
        };
        row = row.child(shown.child(remove(ix, cx)));
    }
    for (name, why) in &c.refused {
        let why = SharedString::from(why.clone());
        row = row.child(
            tile()
                .id(SharedString::from(format!("refused-{name}")))
                .max_w(px(192.))
                .gap_2()
                .pl(px(10.))
                .pr_3()
                .child(Icon::new(Glyph::FileX).with_size(px(16.)).text_color(p.error_foreground))
                .child(
                    v_flex()
                        .min_w_0()
                        .child(div().truncate().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(name.clone()))
                        .child(div().text_size(px(11.)).line_height(px(16.)).text_color(p.error_foreground).child("Refused")),
                )
                .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(why.clone()).build(window, cx)),
        );
    }
    Some(row.into_any_element())
}

/// One 24 px item of the tray: a 12 px glyph and 12 px muted words.
fn tray_item(id: &'static str, p: &Palette) -> Stateful<Div> {
    h_flex()
        .id(id)
        .debug_selector(move || id.into())
        .flex_none()
        .h(px(24.))
        .gap_1()
        .px_2()
        .rounded(px(8.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
}

/// The strip under the box: its own glass below the box's foot, bottom corners 16, holding the computer, the access
/// picker when it is not in the bar, the stash count and the branch.
pub fn tray(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let g = glass(cx);
    let r = c.resolved();
    let in_bar = c.layout() == Layout::Full;
    let mut items = h_flex().relative().h(TRAY).items_center().gap_1().pl(px(6.)).pr_2();
    if let Some(computer) = &c.computer {
        items = items.child(
            tray_item("tray-computer", &p)
                .child(Icon::new(Glyph::Monitor).with_size(px(12.)))
                .child(div().max_w(px(240.)).truncate().child(computer.clone())),
        );
    }
    if !in_bar && r.agent.as_ref().is_some_and(|a| !a.permission_modes.is_empty()) {
        let glyph = access_glyph(r.access.as_ref().map_or("", |a| a.value.as_str()));
        items = items.child(
            div()
                .relative()
                .child(
                    tray_item("tray-access", &p)
                        .cursor_pointer()
                        .hover(|s| s.bg(p.accent).text_color(p.foreground))
                        .when(c.popup == Some(Popup::Access), |el| el.bg(p.accent).text_color(p.foreground))
                        .child(Icon::new(glyph).with_size(px(12.)))
                        .child(r.access_word())
                        .child(Icon::new(Glyph::ChevronDown).with_size(px(12.)).opacity(0.5))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_popup(Popup::Access, cx))),
                )
                .children(menus::picker(c, Popup::Access, cx)),
        );
    }
    if !c.stash.entries.is_empty() {
        items = items.child(
            tray_item("tray-stash", &p)
                .cursor_pointer()
                .hover(|s| s.text_color(p.foreground))
                .child(format!("Stashed {}", c.stash.entries.len()))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_popup(Popup::Stash, cx))),
        );
    }
    items = match &c.branch {
        Some(branch) => items.child(
            tray_item("tray-branch", &p)
                .child(Icon::new(Glyph::GitBranch).with_size(px(12.)))
                .child(div().max_w(px(240.)).truncate().child(branch.clone())),
        ),
        None => items,
    };
    div()
        .debug_selector(|| "composer-tray".into())
        .relative()
        .mx(INSET)
        .h(TRAY)
        .child(
            g.frost(
                div().absolute().inset_0().rounded_b(px(16.)).border_l_1().border_r_1().border_b_1().border_color(g.outline).bg(g.tint),
            ),
        )
        .child(items)
        .into_any_element()
}

/// The line under the box (ChatComposer.tsx `data-composer-under`'s `RefusalSlot`): 26 px in, 13 px in the error ink.
pub fn under(text: String, cx: &App) -> AnyElement {
    div()
        .debug_selector(|| "composer-under".into())
        .mt_2()
        .px(px(26.))
        .min_h(px(36.))
        .text_size(px(13.))
        .line_height(px(18.))
        .text_color(cx.palette().error_foreground)
        .child(text)
        .into_any_element()
}

/// A glass tab over the box, inset 22 px, top corners 16 px, its foot tucked 17 px behind the box (ComposerBanner.tsx).
pub fn attached_surface(menu: bool, cx: &App) -> Div {
    let p = cx.palette();
    let g = glass(cx);
    let (fill, outline) = if menu {
        // Portaled out of the composer, a menu takes the global glass at 80% and the menu's own hairline.
        let fill = p.card.opacity(0.8);
        let outline = p.composer_menu_outline;
        (fill, outline)
    } else {
        (g.tint, g.outline)
    };
    // The tab's shadow falls under its foot, which the web masks away with it, so none is drawn.
    g.frost(div().relative().rounded_t(px(16.)).border_t_1().border_l_1().border_r_1().border_color(outline).bg(fill))
}

/// The fork's or the resume's line over the box.
pub fn banner(b: &Banner, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let glyph = if b.fork { Glyph::Split } else { Glyph::RotateCcwClock };
    div()
        .debug_selector(|| "composer-banner".into())
        .mx(INSET)
        .child(
            attached_surface(false, cx).px_1().pt_1().pb_1().text_size(px(12.)).line_height(px(16.)).child(
                h_flex()
                    .min_h(px(24.))
                    .gap_1()
                    .child(
                        h_flex()
                            .w(px(24.))
                            .flex_none()
                            .justify_center()
                            .child(Icon::new(glyph).with_size(px(12.)).text_color(p.muted_foreground)),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(div().min_w_0().truncate().text_color(p.foreground.opacity(0.8)).child(b.line.clone()))
                            .child(div().flex_none().text_color(p.muted_foreground).child(b.note.clone())),
                    )
                    .child(
                        h_flex()
                            .id("banner-dismiss")
                            .size(px(24.))
                            .justify_center()
                            .rounded(px(8.))
                            .text_color(p.muted_foreground)
                            .cursor_pointer()
                            .hover(|s| s.bg(p.accent).text_color(p.foreground))
                            .child(Icon::new(Glyph::X).with_size(px(14.)))
                            .on_click(cx.listener(|this, _, _, cx| this.set_banner(None, cx))),
                    ),
            ),
        )
        .mb(-px(1.))
        .into_any_element()
}

/// One drawer row (ComposerDrawer.tsx `DrawerRow`): 28 px, its glyph in its ink, its name, its line cut at the edge,
/// a count where it has one, and the chevron up.
#[allow(clippy::too_many_arguments)]
fn drawer_row(
    id: &'static str,
    glyph: Glyph,
    ink: Hsla,
    name: String,
    line: String,
    count: Option<String>,
    mono: &SharedString,
    p: &Palette,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .debug_selector(move || id.into())
        .h(px(28.))
        .gap_2()
        .px_2()
        .rounded(px(10.))
        .hover(|s| s.bg(p.accent))
        .child(Icon::new(glyph).with_size(px(14.)).text_color(ink))
        .child(div().flex_none().text_color(p.muted_foreground).child(name))
        .child(div().flex_1().min_w_0().truncate().text_color(p.foreground.opacity(0.8)).child(line))
        .children(count.map(|n| div().flex_none().font_family(mono.clone()).text_color(p.muted_foreground).child(n)))
        .child(Icon::new(Glyph::ChevronUp).with_size(px(14.)).text_color(p.muted_foreground))
}

/// The drawer on the box's top edge, a row for each thing waiting, most pressing first: the question, the usage limit,
/// the slot, the tasks and the queue. None while nothing waits.
pub fn drawer(c: &Composer, cx: &mut Context<Composer>) -> Option<AnyElement> {
    let d = &c.drawer;
    // The folded prompt's row; the dock stands in the box's place while it is not folded.
    let question = c.dock.as_ref().filter(|dock| dock.folded).map(|dock| dock.prompt.asked.clone());
    if question.is_none() && d.limit.is_none() && d.slot.is_none() && d.tasks.is_none() && c.queue.is_empty() {
        return None;
    }
    let p = cx.palette().clone();
    let g = glass(cx);
    let mono = cx.theme().mono_font_family.clone();
    let mut rows = v_flex();
    if let Some(question) = question {
        rows = rows.child(
            drawer_row(
                "drawer-question",
                Glyph::MessageCircleQuestionMark,
                p.status_input,
                "Waiting for you".into(),
                question,
                None,
                &mono,
                &p,
            )
            .cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| this.unfold_prompt(window, cx))),
        );
    }
    if let Some((name, line)) = &d.limit {
        rows = rows.child(drawer_row("drawer-usage", Glyph::Gauge, p.warning, name.clone(), line.clone(), None, &mono, &p));
    }
    if let Some(line) = &d.slot {
        rows = rows.child(drawer_row(
            "drawer-capped",
            Glyph::Hourglass,
            p.muted_foreground,
            "Waiting for a slot".into(),
            line.clone(),
            None,
            &mono,
            &p,
        ));
    }
    if let Some((step, count)) = &d.tasks {
        rows = rows.child(drawer_row(
            "drawer-tasks",
            Glyph::ListTodo,
            p.muted_foreground,
            "Tasks".into(),
            step.clone(),
            Some(count.clone()),
            &mono,
            &p,
        ));
    }
    if let Some(head) = c.queue.first() {
        let name = model::queue_word(c.queue.len(), c.steering.is_some());
        rows = rows.child(
            drawer_row(
                "drawer-queue",
                Glyph::MessageSquareText,
                p.muted_foreground,
                name,
                model::first_line(&head.prompt).to_owned(),
                None,
                &mono,
                &p,
            )
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.open_bar(super::Bar::Queue, cx))),
        );
    }
    Some(
        div()
            .debug_selector(|| "composer-drawer".into())
            .relative()
            .mx(INSET)
            .mb(px(-1.))
            .rounded_t(px(14.))
            .border_t_1()
            .border_l_1()
            .border_r_1()
            .border_color(g.outline)
            .bg(g.tint)
            .map(|el| g.frost(el))
            .p_1()
            .text_size(px(12.))
            .line_height(px(16.))
            .child(rows)
            .into_any_element(),
    )
}

/// The queue's bar in the composer's place (bars/QueueBar.tsx in Dock.tsx): its head, one card with a row per
/// message holding Edit and Cancel, and the way back to the box.
pub fn queue_bar(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let mut card = v_flex().rounded(px(11.)).border_1().border_color(p.border.opacity(0.6)).bg(p.card.opacity(0.4));
    for (ix, q) in c.queue.iter().enumerate() {
        let id = q.id;
        let files = q.files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ");
        card = card.child(
            h_flex()
                .id(("queue-row", ix))
                .min_h(px(60.))
                .px(px(20.))
                .py_3()
                .gap_3()
                .when(ix > 0, |el| el.border_t_1().border_color(p.border.opacity(0.5)))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_size(px(14.))
                                .line_height(px(20.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.foreground)
                                .child(model::first_line(&q.prompt).to_owned()),
                        )
                        .when(!files.is_empty(), |el| {
                            el.child(div().text_size(px(13.)).line_height(px(18.85)).text_color(p.muted_foreground).child(files))
                        }),
                )
                .child(
                    h_flex()
                        .flex_none()
                        .gap_2()
                        .child(Button::outline(SharedString::from(format!("queue-edit-{id}")), "Edit").xs().on_click({
                            let this = cx.entity().downgrade();
                            move |_, window, cx| _ = this.update(cx, |this, cx| this.edit_queued(id, window, cx))
                        }))
                        .child(Button::outline(SharedString::from(format!("queue-cancel-{id}")), "Cancel").xs().on_click({
                            let this = cx.entity().downgrade();
                            move |_, window, cx| _ = this.update(cx, |this, cx| this.cancel_queued(id, window, cx))
                        })),
                ),
        );
    }
    let head = h_flex()
        .px_5()
        .pt_4()
        .pb_3()
        .gap_2()
        .text_size(px(16.))
        .line_height(px(24.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.foreground)
        .child(Icon::new(Glyph::MessageSquareText).with_size(px(16.)).text_color(p.foreground.opacity(0.8)))
        .child(model::queue_word(c.queue.len(), false));
    let foot = h_flex().px_5().pb_5().child(Button::text("queue-write", "Write a message").on_click({
        let this = cx.entity().downgrade();
        move |_, window, cx| _ = this.update(cx, |this, cx| this.fold_bar(window, cx))
    }));
    v_flex()
        .debug_selector(|| "composer-queue-bar".into())
        .mx_auto()
        .w_full()
        .max_w(super::COLUMN)
        .child(glass_box(cx).child(v_flex().child(head).child(v_flex().px_5().pt_2().pb_5().gap_5().child(card)).child(foot)))
        .into_any_element()
}

/// Files dragged over the composer: the ground at 80% under the glass, and a card saying what a drop takes.
pub fn drop_overlay(cx: &App) -> AnyElement {
    let p = cx.palette();
    let g = glass(cx);
    div()
        .debug_selector(|| "composer-drop".into())
        .absolute()
        .inset_0()
        .flex()
        .p_5()
        .bg(p.background.opacity(0.8))
        .when(crate::glass::composer_blurs(), |el| el.backdrop_blur(g.blur))
        .child(
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .px_6()
                .rounded(px(11.))
                .border_1()
                .border_color(p.border.opacity(0.6))
                .bg(p.foreground.opacity(0.03))
                .child(
                    h_flex()
                        .size(px(32.))
                        .justify_center()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(p.border)
                        .bg(p.foreground.opacity(0.04))
                        .child(Icon::new(Glyph::Paperclip).with_size(px(16.)).text_color(p.foreground.opacity(0.8))),
                )
                .child(
                    v_flex()
                        .max_w(px(320.))
                        .gap_1()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .line_height(px(20.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.foreground)
                                .child("Drop files to attach"),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .line_height(px(20.))
                                .text_color(p.muted_foreground)
                                .text_center()
                                .child("At most 5: an image (PNG, JPEG, GIF or WebP) up to 10 MB, any other file up to 10 MB."),
                        ),
                ),
        )
        .into_any_element()
}
