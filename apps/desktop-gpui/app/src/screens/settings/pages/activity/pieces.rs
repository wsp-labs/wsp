// SPDX-License-Identifier: AGPL-3.0-only
//! Pieces Activity draws that `ui/` does not hold yet, each after the web piece of its name: the grid's cells for a
//! row of its own, a row's menu, the line under a total, a computer's glyph and an agent's mark. Private here until
//! they move to `ui/`.

use std::rc::Rc;

use gpui_kit::component::button::{Button as KitButton, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{
    canvas, div, linear_color_stop, linear_gradient, point, prelude::*, px, AnyElement, App, Bounds, Hsla, PathBuilder, Pixels,
    SharedString, Window,
};
use serde_json::Value;
use wsp_host_client::protocol::PlaceView;

use crate::ui::{ActivePalette, Col, Glyph};

pub type Run = Rc<dyn Fn(&mut Window, &mut App)>;

/// A grid's column gap, `gap-x-4`.
pub const COLUMN_GAP: Pixels = px(16.);

/// The cell boxes of a template, as the grid's own rows lay them: a fill column takes what is left.
pub fn cells(columns: &[Col], items: Vec<AnyElement>) -> Vec<AnyElement> {
    items
        .into_iter()
        .enumerate()
        .map(|(ix, el)| {
            let cell = h_flex().min_w_0();
            match columns.get(ix) {
                Some(Col::Px(w)) => cell.flex_none().w(px(*w)),
                _ => cell.flex_1(),
            }
            .child(el)
            .into_any_element()
        })
        .collect()
}

/// One act of a row's menu, after the web's action registry: its group (a rule stands between groups), glyph, title,
/// whether it destroys, and the word that holds it back while it cannot run.
#[derive(Clone)]
pub struct MenuAct {
    pub group: u8,
    pub glyph: Glyph,
    pub title: SharedString,
    pub destructive: bool,
    pub held: bool,
    pub run: Run,
}

/// The acts as a popup menu, a rule between their groups.
pub fn menu_of(mut menu: PopupMenu, acts: &[MenuAct], danger: Hsla) -> PopupMenu {
    let mut last: Option<u8> = None;
    for act in acts {
        if last.is_some_and(|g| g != act.group) {
            menu = menu.separator();
        }
        last = Some(act.group);
        let run = act.run.clone();
        let item = if act.destructive {
            let title = act.title.clone();
            PopupMenuItem::element(move |_, _| div().text_color(danger).child(title.clone())).icon(Icon::new(act.glyph).text_color(danger))
        } else {
            PopupMenuItem::new(act.title.clone()).icon(Icon::new(act.glyph))
        };
        menu = menu.item(item.disabled(act.held).on_click(move |_, window, cx| run(window, cx)));
    }
    menu
}

/// The row's menu button: an ellipsis in a 20 px square, its acts on a press.
pub fn menu_button(id: SharedString, acts: Rc<Vec<MenuAct>>, cx: &App) -> impl IntoElement {
    let p = cx.palette().clone();
    KitButton::new(id)
        .ghost()
        .with_size(px(20.))
        .rounded(px(4.))
        .text_color(p.muted_foreground)
        .icon(Icon::new(Glyph::Ellipsis).with_size(px(14.)))
        .dropdown_menu(move |menu, _, _| menu_of(menu, &acts, p.error_foreground))
}

/// `ChartPlot` at a tile's size: the rule lines, the run as a line over a faint fill, no end dot. Points are 0 to 1
/// across and 0 to 1 up.
pub fn sparkline(points: Vec<(f32, f32)>, ink: Hsla, rule: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, _, window: &mut Window, _| {
            let (w, h) = (bounds.size.width, bounds.size.height);
            let pad = h * (1. / 96.);
            let y_at = |v: f32| bounds.origin.y + pad + (h - pad * 2.) * (1. - v);
            for (at, dashed) in [(0.25, true), (0.5, true), (0.75, true), (1., false)] {
                let mut line = if dashed { PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(3.)]) } else { PathBuilder::stroke(px(1.)) };
                line.move_to(point(bounds.origin.x, y_at(at)));
                line.line_to(point(bounds.origin.x + w, y_at(at)));
                if let Ok(path) = line.build() {
                    window.paint_path(path, rule);
                }
            }
            let mut base = PathBuilder::stroke(px(1.));
            base.move_to(point(bounds.origin.x, bounds.origin.y + h - pad));
            base.line_to(point(bounds.origin.x + w, bounds.origin.y + h - pad));
            if let Ok(path) = base.build() {
                window.paint_path(path, rule);
            }
            if points.is_empty() {
                return;
            }
            let at = |(x, y): (f32, f32)| point(bounds.origin.x + w * x, y_at(y));
            if points.len() > 1 {
                let mut area = PathBuilder::fill();
                area.move_to(at(points[0]));
                for pt in &points[1..] {
                    area.line_to(at(*pt));
                }
                area.line_to(point(bounds.origin.x + w * points[points.len() - 1].0, bounds.origin.y + h));
                area.line_to(point(bounds.origin.x + w * points[0].0, bounds.origin.y + h));
                area.close();
                if let Ok(path) = area.build() {
                    window.paint_path(
                        path,
                        linear_gradient(180., linear_color_stop(ink.opacity(0.2), 0.), linear_color_stop(ink.opacity(0.), 1.)),
                    );
                }
            }
            let mut stroke = PathBuilder::stroke(px(1.5));
            stroke.move_to(at(points[0]));
            for pt in points.iter().skip(1) {
                stroke.line_to(at(*pt));
            }
            if points.len() == 1 {
                stroke.line_to(at(points[0]));
            }
            if let Ok(path) = stroke.build() {
                window.paint_path(path, ink);
            }
        },
    )
    .w_full()
    .h(px(40.))
}

/// `ComputerGlyph`: the person's pick off the row's menu, else what the computer is.
pub fn computer_glyph(place: &PlaceView, look: Option<&Value>) -> Glyph {
    let picked = look.and_then(|l| l.get(&place.id)).and_then(|l| l.get("icon")).and_then(Value::as_str).map(str::to_owned);
    let facts = serde_json::to_value(place).unwrap_or(Value::Null);
    let icon = picked.unwrap_or_else(|| {
        if facts.get("kind").and_then(Value::as_str) == Some("provider") {
            return "cloud".into();
        }
        match facts.get("mac").and_then(Value::as_str) {
            Some("macbook") => "laptop".into(),
            Some("imac" | "mac-mini" | "mac-studio") => "desktop".into(),
            Some("mac-pro") => "tower".into(),
            _ if place.id == "here" => "desktop".into(),
            _ => "server".into(),
        }
    });
    match icon.as_str() {
        "laptop" => Glyph::Laptop,
        "tower" => Glyph::PcCase,
        "server" => Glyph::Server,
        "cloud" => Glyph::Cloud,
        "cpu" => Glyph::Cpu,
        "drive" => Glyph::HardDrive,
        "container" => Glyph::Box,
        "home" => Glyph::House,
        _ => Glyph::Monitor,
    }
}

/// `HarnessMark`'s stand-in until the agents page's marks land in `ui/`: the agent glyph at the mark's size.
pub fn harness_mark(_id: &str, _label: &str, size: Pixels, cx: &App) -> AnyElement {
    Icon::new(Glyph::Bot).with_size(size).flex_none().text_color(cx.palette().foreground.opacity(0.8)).into_any_element()
}
