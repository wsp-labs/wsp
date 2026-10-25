// SPDX-License-Identifier: AGPL-3.0-only
//! The composer in a test window the size of the web's thread column, put into one of `states`, as the layout and
//! keys tests open it.
// Each test crate that includes this reads only some of it.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use desktop_gpui::screens::composer::screen::column;
use desktop_gpui::screens::composer::{self, Composer, ComposerEvent};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{prelude::*, px, size, Context, Entity, TestAppContext, VisualTestContext, Window};

use super::states;

pub struct Column {
    pub composer: Entity<Composer>,
}

impl Render for Column {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fresh = self.composer.read(cx).fresh();
        column(fresh, self.composer.clone())
    }
}

pub fn open<'a>(state: &str, cx: &'a mut TestAppContext) -> &'a mut VisualTestContext {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
    });
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let composer = cx.new(|cx| Composer::new(window, cx));
        made = Some(composer.clone());
        let column = cx.new(|_| Column { composer });
        Root::new(column, window, cx)
    });
    cx.simulate_resize(size(px(1024.), px(800.)));
    let composer = made.unwrap();
    composer.update_in(cx, |c, window, cx| states::stage(state, c, window, cx));
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
    cx
}

pub fn composer_of(cx: &mut VisualTestContext) -> Entity<Composer> {
    cx.update(|window, cx| {
        let root = window.root::<Root>().flatten().unwrap();
        let column = root.read(cx).view().clone().downcast::<Column>().unwrap();
        column.read(cx).composer.clone()
    })
}

/// The prompts of every new turn the composer asks the host for.
pub fn starts(composer: &Entity<Composer>, cx: &mut VisualTestContext) -> Rc<RefCell<Vec<String>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    cx.update(|_, cx| {
        cx.subscribe(composer, move |_, event: &ComposerEvent, _| {
            if let ComposerEvent::Start(out) = event {
                sink.borrow_mut().push(out.prompt.clone());
            }
        })
        .detach()
    });
    seen
}
