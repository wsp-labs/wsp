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
use crate::ui::{catalog, tagged, ActivePalette, Button, Glyph, Grid, Palette, Step, StepRow};

const RADIUS: Pixels = px(22.);
/// `--chat-composer-drawer-inset`: how far the tray, the drawer, the banner and the menus sit in from the box's sides.
pub const INSET: Pixels = px(22.);
/// What shows of the tray under the box: `--chat-composer-tray-height`, 36 px under 640 px.
pub const TRAY: Pixels = px(32.);
const TRAY_NARROW: Pixels = px(36.);

/// The glass (index.css and ComposerSurface.tsx): the tint, the hairline and the dark side's top light. The light side's
/// drop shadow, the theme's `composer_shadow`, is not drawn: GPUI skips a shadow whose own rect lies outside its clip,
/// and unclipped it shows through the glass.
pub struct Glass {
    pub tint: Hsla,
    pub outline: Hsla,
    pub highlight: Hsla,
    pub blur: Pixels,
    pub saturation: f32,
    /// What the blurred copy is laid over, on the Mac.
    pub ground: Option<Hsla>,
}

impl Glass {
    /// The blur and its saturation over what lies under `el`, on its ground, unless the composer's glass is turned off.
    pub fn frost<E: Styled>(&self, el: E) -> E {
        if crate::glass::composer_blurs() {
            let el = el.backdrop_blur(self.blur).backdrop_saturation(self.saturation);
            match self.ground {
                Some(ground) => el.backdrop_ground(ground),
                None => el,
            }
        } else {
            el
        }
    }
}

pub fn glass(cx: &App) -> Glass {
    let p = cx.palette();
    let dark = cx.theme().is_dark();
    let look = cx.global::<crate::ui::theme::WspTheme>().look;
    let ground = look.mac.then(|| crate::glass::mac_ground(dark));
    let mut glass = if dark {
        Glass {
            tint: p.foreground.opacity(0.03),
            outline: p.composer_outline,
            highlight: p.composer_highlight,
            blur: px(16.),
            saturation: 1.08,
            ground,
        }
    } else {
        Glass {
            tint: p.card.opacity(0.55),
            outline: p.composer_outline,
            highlight: p.composer_highlight,
            blur: px(12.),
            saturation: 1.14,
            ground,
        }
    };
    // `:root:root.solid [data-slot="composer-shell"]`: a page drawing no glass gives the composer the theme's card, whole.
    if !look.glass {
        glass.tint = p.card;
    }
    glass
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
    if c.access_in_bar() && !agent.permission_modes.is_empty() {
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
    // `size-9 sm:size-8`; the stop alone keeps 32 px at every width (`size-8 sm:h-8 sm:w-8`).
    let send_size = if c.narrow { px(36.) } else { px(32.) };
    let circle = |id: &'static str, size: Pixels| {
        h_flex().id(id).debug_selector(move || id.into()).relative().flex_none().justify_center().size(size).rounded_full()
    };
    let main = if c.running {
        let stopping = c.stopping;
        circle("composer-stop", px(32.))
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
                circle("composer-send", send_size)
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
                    .tooltip(crate::ui::tip(words))
                    .into_any_element()
            }
            None if c.banner.as_ref().is_some_and(|b| b.resume.is_some()) => h_flex()
                .id("composer-send")
                .debug_selector(|| "composer-send".into())
                .relative()
                .flex_none()
                .h(send_size)
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
            None => circle("composer-send", send_size)
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
    let mut row = h_flex()
        .debug_selector(|| "composer-files".into())
        .flex_wrap()
        .gap(px(6.))
        .map(|el| if c.narrow { el.px_3() } else { el.px_4() })
        .pt_3();
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
                .tooltip(crate::ui::tip(why)),
        );
    }
    Some(row.into_any_element())
}

/// One 24 px item of the tray, 28 px under 640 px (`ROW_ITEM_CLASS`): a 12 px glyph and 12 px muted words.
fn tray_item(id: &'static str, narrow: bool, p: &Palette) -> Stateful<Div> {
    h_flex()
        .id(id)
        .debug_selector(move || id.into())
        .flex_none()
        .h(if narrow { px(28.) } else { px(24.) })
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
    let in_bar = c.access_in_bar();
    let tray = if c.narrow { TRAY_NARROW } else { TRAY };
    let mut items = h_flex().relative().h(tray).items_center().gap_1().pl(px(6.)).pr_2();
    if let Some(computer) = &c.computer {
        items = items.child(
            tray_item("tray-computer", c.narrow, &p)
                .children(c.computer_icon.map(|icon| icon.draw(p.muted_foreground, px(12.))))
                .child(div().debug_selector(|| format!("tray-computer:{computer}")).max_w(px(240.)).truncate().child(computer.clone())),
        );
    }
    if !in_bar && r.agent.as_ref().is_some_and(|a| !a.permission_modes.is_empty()) {
        let glyph = access_glyph(r.access.as_ref().map_or("", |a| a.value.as_str()));
        items = items.child(
            div()
                .relative()
                .child(
                    tray_item("tray-access", c.narrow, &p)
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
            tray_item("tray-stash", c.narrow, &p)
                .cursor_pointer()
                .hover(|s| s.text_color(p.foreground))
                .child(format!("Stashed {}", c.stash.entries.len()))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_popup(Popup::Stash, cx))),
        );
    }
    // A fork's draft names the new branch its send opens the thread on (`starts`), which no folder holds yet.
    items = match c.forking.as_ref().and_then(|f| f.branch.clone()).or_else(|| c.project_branch.clone()).or_else(|| c.branch.clone()) {
        Some(branch) => items.child(
            tray_item("tray-branch", c.narrow, &p)
                .child(Icon::new(Glyph::GitBranch).with_size(px(12.)))
                .child(div().debug_selector(|| format!("tray-branch:{branch}")).max_w(px(240.)).truncate().child(branch)),
        ),
        None => items,
    };
    div()
        .debug_selector(|| "composer-tray".into())
        .relative()
        .mx(INSET)
        .h(tray)
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
                            .debug_selector(|| "banner-dismiss".into())
                            .aria_label(b.dismiss())
                            .size(px(24.))
                            .justify_center()
                            .rounded(px(8.))
                            .text_color(p.muted_foreground)
                            .cursor_pointer()
                            .hover(|s| s.bg(p.accent).text_color(p.foreground))
                            .child(Icon::new(Glyph::X).with_size(px(14.)))
                            .on_click(cx.listener(|this, _, window, cx| this.cancel_fork(window, cx))),
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
    if let Some(limit) = &d.limit {
        rows = rows.child(
            drawer_row("drawer-usage", Glyph::Gauge, p.warning, limit.title.clone(), limit.line.clone(), None, &mono, &p)
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.open_bar(super::Bar::Usage, cx))),
        );
    }
    if let Some(slot) = &d.slot {
        rows = rows.child(
            drawer_row(
                "drawer-capped",
                Glyph::Hourglass,
                p.muted_foreground,
                "Waiting for a slot".into(),
                slot.line.clone(),
                None,
                &mono,
                &p,
            )
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.open_bar(super::Bar::Capped, cx))),
        );
    }
    if let Some(tasks) = &d.tasks {
        rows = rows.child(
            drawer_row(
                "drawer-tasks",
                Glyph::ListTodo,
                p.muted_foreground,
                "Tasks".into(),
                tasks.step.clone(),
                Some(tasks.count()),
                &mono,
                &p,
            )
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.open_bar(super::Bar::Tasks, cx))),
        );
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

/// What stands in the composer's place in the dock's frame (Dock.tsx): the agent's question or a bar; its mark,
/// title, the lines under the title and a quiet aside at the head's right, its cards, the way back at the foot's left
/// and its acts at the right.
pub struct DockParts {
    pub selector: &'static str,
    pub mark: Option<AnyElement>,
    pub title: SharedString,
    pub under: Vec<AnyElement>,
    pub aside: Option<SharedString>,
    /// The aside in the mono face: the Tasks bar's done of all (`font-mono`), where the question's count is plain `FACT`.
    pub aside_mono: bool,
    pub body: Vec<AnyElement>,
    /// The foot's way back; none for a bar's Write a message, which folds it.
    pub back: Option<AnyElement>,
    pub acts: Vec<AnyElement>,
    /// The frame's inside, where the question takes its keys; none for a bar's plain column.
    pub root: Option<Stateful<Div>>,
}

/// The dock's frame (Dock.tsx, `STEP_HEAD`, `STEP_BODY`, `DOCK_FOOT`): the glass box in the composer's column; a head of
/// the mark and a 16 px semibold title, the lines under it and the aside at the right; the cards; the foot.
pub fn dock_frame(c: &Composer, parts: DockParts, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let head = h_flex()
        .items_start()
        .justify_between()
        .gap_6()
        .px_5()
        .pt_4()
        .pb_3()
        .child(
            v_flex()
                .min_w_0()
                .gap_1()
                .child(
                    h_flex()
                        .debug_selector(|| "dock-title".into())
                        .min_w_0()
                        .items_center()
                        .gap_2()
                        .text_size(px(16.))
                        .line_height(px(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.foreground)
                        .children(parts.mark)
                        .child(div().min_w_0().child(parts.title)),
                )
                .children(parts.under),
        )
        .children(parts.aside.map(|aside| {
            div()
                .debug_selector(|| "dock-aside".into())
                .when(parts.aside_mono, |el| el.font_family(cx.theme().mono_font_family.clone()))
                .flex_none()
                .text_size(px(13.))
                .line_height(px(19.5))
                .text_color(p.muted_foreground)
                .child(aside)
        }));
    let back = parts.back.unwrap_or_else(|| {
        let this = cx.entity().downgrade();
        let back = Button::text("dock-back", "Write a message")
            .on_click(move |_, window, cx| _ = this.update(cx, |this, cx| this.fold_bar(window, cx)));
        tagged("dock-back", back)
    });
    let foot = dock_foot(c.narrow, back, parts.acts);
    let body = parts.body;
    let inside = parts
        .root
        .unwrap_or_else(|| v_flex().id("dock-root"))
        .child(head)
        .when(!body.is_empty(), |el| el.child(v_flex().px_5().pt_2().pb_5().gap_5().children(body)))
        .child(foot);
    let selector = parts.selector;
    v_flex()
        .debug_selector(move || selector.into())
        .mx_auto()
        .w_full()
        .max_w(super::COLUMN)
        .child(glass_box(cx).child(inside))
        .into_any_element()
}

/// The dock's foot (`StepFoot` with `DOCK_FOOT`): the way back at the left and the acts at the right, the primary last;
/// under 640 px the acts stack across the foot with the primary on top and the way back centred under them.
fn dock_foot(narrow: bool, back: AnyElement, acts: Vec<AnyElement>) -> Div {
    let foot = if narrow { v_flex() } else { h_flex().items_center() };
    let foot = foot.debug_selector(|| "dock-foot".into()).gap_2().px_5().pb_5();
    if narrow {
        foot.children(acts.into_iter().rev()).child(h_flex().justify_center().min_h(px(20.)).child(back))
    } else {
        foot.child(h_flex().flex_1().min_w_0().min_h(px(20.)).child(back)).children(acts)
    }
}

/// `GLYPH`: the head's 16 px mark at 80% ink.
fn mark(glyph: Glyph, ink: Hsla) -> AnyElement {
    Icon::new(glyph).with_size(px(16.)).text_color(ink).into_any_element()
}

/// The quiet sentence under a bar's title (`NOTE`).
pub(super) fn note(text: String, p: &Palette) -> Div {
    div().debug_selector(|| "dock-note".into()).text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(text)
}

/// The queue's bar in the composer's place (bars/QueueBar.tsx): one card with a row per message holding Edit and
/// Cancel.
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
    dock_frame(
        c,
        DockParts {
            selector: "composer-queue-bar",
            mark: Some(mark(Glyph::MessageSquareText, p.foreground.opacity(0.8))),
            title: model::queue_word(c.queue.len(), false).into(),
            under: Vec::new(),
            aside: None,
            aside_mono: false,
            body: vec![card.into_any_element()],
            acts: Vec::new(),
            back: None,
            root: None,
        },
        cx,
    )
}

/// The agent's step list in the composer's place (bars/TasksBar.tsx): Tasks with done of all at the head's right,
/// then a card of the steps, each with its mark and how long it took, the step at work ticking.
pub fn tasks_bar(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let Some(tasks) = c.drawer.tasks.clone() else { return div().into_any_element() };
    let rows = tasks.steps.iter().enumerate().map(|(ix, step)| {
        let state = match step.state {
            model::StepState::Done => Step::Done,
            model::StepState::Working => Step::Running,
            model::StepState::Pending => Step::NotStarted,
        };
        let row = StepRow::new(SharedString::from(format!("task-{ix}")), state, step.text.clone()).agent(true);
        match (step.took, step.since) {
            (Some(ms), _) => row.took(ms),
            (None, Some(at)) => row.since(at),
            (None, None) => row,
        }
    });
    dock_frame(
        c,
        DockParts {
            selector: "composer-tasks-bar",
            mark: Some(mark(Glyph::ListTodo, p.foreground.opacity(0.8))),
            title: "Tasks".into(),
            under: Vec::new(),
            aside: Some(tasks.count().into()),
            aside_mono: true,
            body: vec![Grid::new("composer-tasks").rows(rows).into_any_element()],
            acts: Vec::new(),
            back: None,
            root: None,
        },
        cx,
    )
}

/// A turn the agent's usage limit stopped, in the composer's place (bars/UsageBar.tsx): the state as the title with
/// the agent's sentence under it, then Cancel and Resume at reset, none where the agent named no reset.
pub fn usage_bar(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let Some(limit) = c.drawer.limit.clone() else { return div().into_any_element() };
    let this = cx.entity().downgrade();
    let acts = if limit.resets {
        let (cancel, resume) = (this.clone(), this);
        let armed = limit.armed;
        vec![
            tagged(
                "limit-cancel",
                Button::outline("limit-cancel", "Cancel").on_click(move |_, window, cx| {
                    _ = cancel.update(cx, |c, cx| if armed { c.mark_resume(false, window, cx) } else { c.fold_bar(window, cx) })
                }),
            ),
            tagged(
                "limit-resume",
                Button::primary("limit-resume", "Resume at reset")
                    .held(armed)
                    .on_click(move |_, window, cx| _ = resume.update(cx, |c, cx| c.mark_resume(true, window, cx))),
            ),
        ]
    } else {
        Vec::new()
    };
    dock_frame(
        c,
        DockParts {
            selector: "composer-usage-bar",
            mark: Some(mark(Glyph::Gauge, p.warning)),
            title: limit.title.clone().into(),
            under: vec![note(limit.note.clone(), &p).into_any_element()],
            aside: None,
            aside_mono: false,
            body: Vec::new(),
            acts,
            back: None,
            root: None,
        },
        cx,
    )
}

/// A turn its computer's threads at once holds back, in the composer's place (bars/CapBar.tsx): the computer it waits
/// on as the title, what runs there and how the wait ends under it, then the setting that lets it start now.
pub fn cap_bar(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let Some(slot) = c.drawer.slot.clone() else { return div().into_any_element() };
    let acts = slot.raise.clone().map(|raise| {
        let route = crate::screens::Route::Screen(format!("settings:computer:{}", slot.place_id).into());
        tagged("cap-raise", Button::primary("cap-raise", raise).on_click(move |_, _, cx| crate::screens::navigate(route.clone(), cx)))
    });
    dock_frame(
        c,
        DockParts {
            selector: "composer-cap-bar",
            mark: Some(mark(Glyph::Hourglass, p.muted_foreground)),
            title: slot.title.clone().into(),
            under: vec![note(slot.note.clone(), &p).into_any_element()],
            aside: None,
            aside_mono: false,
            body: Vec::new(),
            acts: acts.into_iter().collect(),
            back: None,
            root: None,
        },
        cx,
    )
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

#[cfg(test)]
mod tests {
    use gpui_kit::TestAppContext;

    use crate::screens::app_shell::material::Look;
    use crate::ui::{self, theme, ActivePalette, Side};

    /// `ComposerSurface`'s tint on the glass, and `:root:root.solid`'s card, whole, where the page draws none.
    #[gpui_kit::test]
    fn the_composer_takes_the_card_whole_where_the_page_draws_no_glass(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Dark, cx);
            for id in ["graphite", "paper"] {
                theme::set_theme_in(id, Look { mac: false, glass: false }, cx);
                assert_eq!(super::glass(cx).tint, cx.palette().card, "{id} with no glass");
                theme::set_theme_in(id, Look { mac: false, glass: true }, cx);
                let p = cx.palette();
                let tint = if id == "graphite" { p.foreground.opacity(0.03) } else { p.card.opacity(0.55) };
                assert_eq!(super::glass(cx).tint, tint, "{id} on the glass");
            }
        });
    }
}
