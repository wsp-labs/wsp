// SPDX-License-Identifier: AGPL-3.0-only
//! The lucide glyphs the pieces draw. gpui-kit's default bundle holds only its components' own, so the app's asset
//! source serves these on top of it.

use std::borrow::Cow;

use gpui_kit::assets::{icon_assets, Assets, IconName};
use gpui_kit::{AssetSource, Result, SharedString};

icon_assets!(
    Glyphs,
    [
        Bookmark,
        Brain,
        Check,
        ChevronDown,
        ChevronRight,
        ChevronUp,
        Circle,
        CircleAlert,
        CircleCheck,
        CircleMinus,
        CircleSlash,
        Cloud,
        Copy,
        File,
        FileText,
        FileX,
        Folder,
        GitBranch,
        Hand,
        RotateCcwClock,
        Hourglass,
        KeyRound,
        LoaderCircle,
        Lock,
        LockOpen,
        MessageCircleQuestionMark,
        MessageSquareText,
        Monitor,
        Paperclip,
        PenLine,
        PencilRuler,
        Plus,
        RotateCcw,
        Search,
        Settings,
        Shield,
        Sparkles,
        Split,
        Star,
        Terminal,
        X,
        Zap,
    ]
);

/// The checkbox's tick, which is not lucide's: a 3 px stroke on its own path.
pub const TICK: &str = "icons/wsp-tick.svg";
const TICK_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5.252 12.7 10.2 18.63 18.748 5.37"/></svg>"#;

/// The composer's send arrow and stop square, which are its own paths and not lucide's (ComposerPrimaryActions.tsx).
pub const SEND: &str = "icons/wsp-send.svg";
const SEND_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M7 11.5V2.5M7 2.5L3 6.5M7 2.5L11 6.5"/></svg>"#;
pub const STOP: &str = "icons/wsp-stop.svg";
const STOP_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="currentColor"><rect x="2" y="2" width="8" height="8" rx="1.5"/></svg>"#;

/// The glyphs a piece names, by lucide's own names.
pub type Glyph = IconName;

/// gpui-kit's bundle with the pieces' glyphs over it; open the application with it.
#[derive(Clone, Copy, Debug, Default)]
pub struct UiAssets;

impl AssetSource for UiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        for (own, svg) in [(TICK, TICK_SVG), (SEND, SEND_SVG), (STOP, STOP_SVG)] {
            if path == own {
                return Ok(Some(Cow::Borrowed(svg)));
            }
        }
        match Glyphs.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> = [TICK, SEND, STOP].into_iter().filter(|p| p.starts_with(path)).map(Into::into).collect();
        paths.extend(Glyphs.list(path)?);
        paths.extend(Assets.list(path)?);
        Ok(paths)
    }
}
