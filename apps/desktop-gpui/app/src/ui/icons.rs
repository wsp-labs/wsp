// SPDX-License-Identifier: AGPL-3.0-only
//! The lucide glyphs the pieces draw. gpui-kit's default bundle holds only its components' own, so the app's asset
//! source serves these on top of it.

use std::borrow::Cow;

use gpui_kit::assets::{icon_assets, Assets, IconName};
use gpui_kit::{AssetSource, Result, SharedString};

icon_assets!(
    Glyphs,
    [
        ArrowDown,
        Bot,
        Brain,
        Check,
        ChevronDown,
        ChevronRight,
        ChevronUp,
        Circle,
        CircleAlert,
        CircleCheck,
        CircleMinus,
        Cloud,
        Copy,
        Eye,
        File,
        FilePlus,
        FileText,
        Folder,
        Globe,
        Hammer,
        Hourglass,
        Info,
        KeyRound,
        LoaderCircle,
        MessageCircleQuestionMark,
        Minimize2,
        Monitor,
        Play,
        Plus,
        RotateCcw,
        Search,
        Settings,
        Split,
        SquarePen,
        Terminal,
        Undo2,
        Wrench,
        Zap,
    ]
);

/// The checkbox's tick, which is not lucide's: a 3 px stroke on its own path.
pub const TICK: &str = "icons/wsp-tick.svg";
const TICK_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5.252 12.7 10.2 18.63 18.748 5.37"/></svg>"#;

/// The glyphs a piece names, by lucide's own names.
pub type Glyph = IconName;

/// gpui-kit's bundle with the pieces' glyphs over it; open the application with it.
#[derive(Clone, Copy, Debug, Default)]
pub struct UiAssets;

impl AssetSource for UiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == TICK {
            return Ok(Some(Cow::Borrowed(TICK_SVG)));
        }
        match Glyphs.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> = [TICK].into_iter().filter(|p| p.starts_with(path)).map(Into::into).collect();
        paths.extend(Glyphs.list(path)?);
        paths.extend(Assets.list(path)?);
        Ok(paths)
    }
}
