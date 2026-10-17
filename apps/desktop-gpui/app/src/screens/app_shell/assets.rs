// SPDX-License-Identifier: AGPL-3.0-only
//! The frame's own glyphs and the wordmark, served over the shared pieces' bundle; the app opens with [`AppAssets`].

use std::borrow::Cow;

use gpui_kit::assets::icon_assets;
use gpui_kit::{AssetSource, Result, SharedString};

use crate::ui::UiAssets;

icon_assets!(
    FrameGlyphs,
    [
        Activity,
        Bot,
        Cpu,
        FileDiff,
        Folder,
        GitPullRequest,
        Globe,
        MessageSquareDashed,
        PanelBottom,
        PanelLeft,
        PanelLeftClose,
        PanelRight,
        PanelsTopLeft,
        Plus,
        SquarePen,
        SquareTerminal,
        X,
    ]
);

/// `apps/web/src/brand/lockup.svg`, whose strokes take the element's text color.
pub const LOCKUP: &str = "brand/lockup.svg";
const LOCKUP_SVG: &[u8] = include_bytes!("../../../../../web/src/brand/lockup.svg");

#[derive(Clone, Copy, Debug, Default)]
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == LOCKUP {
            return Ok(Some(Cow::Borrowed(LOCKUP_SVG)));
        }
        match FrameGlyphs.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => UiAssets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> = [LOCKUP].into_iter().filter(|p| p.starts_with(path)).map(Into::into).collect();
        paths.extend(FrameGlyphs.list(path)?);
        paths.extend(UiAssets.list(path)?);
        Ok(paths)
    }
}
