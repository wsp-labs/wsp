// SPDX-License-Identifier: AGPL-3.0-only
//! The frame's own glyphs, served over the shared pieces' bundle, which holds the wordmark; the app opens with [`AppAssets`].

use std::borrow::Cow;

use gpui_kit::assets::icon_assets;
use gpui_kit::{AssetSource, Result, SharedString};

use crate::ui::UiAssets;

// The frame's glyphs gpui-kit's default bundle lacks; the rest it already serves.
icon_assets!(FrameGlyphs, [Activity, Earth, FileDiff, GitPullRequest, MessageSquareDashed, PanelsTopLeft, SquarePen, X]);

#[derive(Clone, Copy, Debug, Default)]
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(svg) = crate::screens::pr_pane::files::rows::glyph_svg(path) {
            return Ok(Some(Cow::Borrowed(svg.as_bytes())));
        }
        match FrameGlyphs.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => UiAssets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = FrameGlyphs.list(path)?;
        paths.extend(UiAssets.list(path)?);
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_webs_lockup_is_served_at_one_path() {
        let web = include_bytes!("../../../../../web/src/brand/lockup.svg");
        let paths = AppAssets.list("").unwrap();
        let serving: Vec<_> = paths.iter().filter(|p| AppAssets.load(p).ok().flatten().is_some_and(|b| b.as_ref() == web)).collect();
        assert_eq!(serving, [&SharedString::from(crate::ui::LOCKUP)]);
    }
}
