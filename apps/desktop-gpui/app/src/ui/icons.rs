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
        CircleDot,
        CircleMinus,
        CircleSlash,
        Cloud,
        Copy,
        File,
        FileText,
        FileX,
        Gauge,
        Folder,
        GitBranch,
        GitPullRequest,
        Hand,
        Hourglass,
        KeyRound,
        ListTodo,
        LoaderCircle,
        Lock,
        LockOpen,
        MessageCircleQuestionMark,
        MessageSquareText,
        Monitor,
        Package,
        Paperclip,
        PenLine,
        PencilRuler,
        Plus,
        RotateCcw,
        RotateCcwClock,
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
/// lucide's star, filled, for a starred model.
pub const STAR_FILLED: &str = "icons/wsp-star-filled.svg";
const STAR_FILLED_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/></svg>"#;
const STOP_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="currentColor"><rect x="2" y="2" width="8" height="8" rx="1.5"/></svg>"#;

/// The glyphs a piece names, by lucide's own names.
pub type Glyph = IconName;

/// gpui-kit's bundle with the pieces' glyphs over it; open the application with it.
#[derive(Clone, Copy, Debug, Default)]
pub struct UiAssets;

impl AssetSource for UiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        for (own, svg) in [(TICK, TICK_SVG), (SEND, SEND_SVG), (STOP, STOP_SVG), (STAR_FILLED, STAR_FILLED_SVG)] {
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
        let mut paths: Vec<SharedString> =
            [TICK, SEND, STOP, STAR_FILLED].into_iter().filter(|p| p.starts_with(path)).map(Into::into).collect();
        paths.extend(Glyphs.list(path)?);
        paths.extend(Assets.list(path)?);
        Ok(paths)
    }
}
