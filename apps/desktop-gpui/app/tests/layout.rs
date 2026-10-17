// SPDX-License-Identifier: AGPL-3.0-only
use desktop_gpui::Home;
use gpui::{point, px, size, AppContext, Bounds, TestAppContext};
use gpui_component::Root;

#[gpui::test]
fn the_title_sits_in_the_middle_of_the_window(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(cx.new(|_| Home), window, cx));
    cx.simulate_resize(size(px(900.), px(600.)));
    cx.run_until_parked();

    let home = cx.debug_bounds("home").expect("the view was not drawn");
    assert_eq!(home, Bounds::new(point(px(0.), px(0.)), size(px(900.), px(600.))));

    let title = cx.debug_bounds("title").expect("the title was not drawn");
    assert!(title.size.width > px(0.) && title.size.height > px(0.), "the title has no box: {title:?}");
    assert!(title.size.height < home.size.height / 2., "the title's box is not its line of text: {title:?}");
    // Text is laid out on the half-pixel grid, so its center can sit up to half a pixel off.
    let offset = title.center() - home.center();
    assert!(offset.x.abs() <= px(0.5) && offset.y.abs() <= px(0.5), "the title is off center by {offset:?}");
}
