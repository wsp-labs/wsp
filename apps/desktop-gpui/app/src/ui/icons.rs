// SPDX-License-Identifier: AGPL-3.0-only
//! The lucide glyphs the pieces draw. gpui-kit's default bundle holds only its components' own, so the app's asset
//! source serves these on top of it.

use std::borrow::Cow;

use gpui_kit::assets::{icon_assets, Assets, IconName};
use gpui_kit::{AssetSource, Result, SharedString};

icon_assets!(
    Glyphs,
    [
        Activity,
        AppWindow,
        ArrowDown,
        ArrowLeft,
        Book,
        Bot,
        Box,
        Camera,
        ChartLine,
        Check,
        ChevronDown,
        ChevronLeft,
        ChevronRight,
        Circle,
        CircleAlert,
        CircleArrowUp,
        CircleCheck,
        CircleDashed,
        CircleMinus,
        CircleStop,
        ClipboardCopy,
        Clock,
        Cloud,
        Code,
        Cog,
        Computer,
        Container,
        Copy,
        Cpu,
        Database,
        Download,
        Earth,
        Ellipsis,
        EthernetPort,
        ExternalLink,
        File,
        Flame,
        FlaskConical,
        Folder,
        Gamepad2,
        Gauge,
        GitCommitHorizontal,
        Github,
        Globe,
        GripVertical,
        Hammer,
        HardDrive,
        Heart,
        Hourglass,
        House,
        Image,
        KeyRound,
        Keyboard,
        Laptop,
        Leaf,
        ListChecks,
        LoaderCircle,
        LogIn,
        MessageCircleQuestionMark,
        MessageSquare,
        MessagesSquare,
        Minus,
        Monitor,
        Music,
        Palette,
        PanelLeft,
        PanelLeftClose,
        PanelsTopLeft,
        PcCase,
        Pencil,
        Plug,
        Plus,
        Power,
        PowerOff,
        Puzzle,
        RefreshCw,
        Rocket,
        RotateCcw,
        RotateCw,
        ScrollText,
        Search,
        Server,
        Settings,
        Shield,
        SlidersHorizontal,
        Smartphone,
        SquareTerminal,
        SquareX,
        Star,
        Tag,
        Terminal,
        Trash,
        TriangleAlert,
        User,
        UserRound,
        Wrench,
        Zap,
    ]
);

/// The checkbox's tick, which is not lucide's: a 3 px stroke on its own path.
pub const TICK: &str = "icons/wsp-tick.svg";
const TICK_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5.252 12.7 10.2 18.63 18.748 5.37"/></svg>"#;

/// wsp's lockup, the tilde beside the wordmark: the web's own file, so the two cannot drift.
pub const LOCKUP: &str = "icons/wsp-lockup.svg";
const LOCKUP_SVG: &[u8] = include_bytes!("../../../../web/src/brand/lockup.svg");

/// `--surface-grain`, the 256 px noise tile the web lays over the body and the sidebar, drawn by Chromium from the
/// web's own SVG at 3.5% and kept with its alpha.
pub const GRAIN: &str = "images/wsp-grain.png";
const GRAIN_PNG: &[u8] = include_bytes!("grain.png");

/// wsp's mark, the tilde alone: the Version row's lead.
pub const MARK: &str = "icons/wsp-mark.svg";
const MARK_SVG: &[u8] = include_bytes!("../../../../web/src/brand/mark.svg");

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
        if path == MARK {
            return Ok(Some(Cow::Borrowed(MARK_SVG)));
        }
        if path == LOCKUP {
            return Ok(Some(Cow::Borrowed(LOCKUP_SVG)));
        }
        if path == GRAIN {
            return Ok(Some(Cow::Borrowed(GRAIN_PNG)));
        }
        match Glyphs.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> =
            [TICK, MARK, LOCKUP, GRAIN].into_iter().filter(|p| p.starts_with(path)).map(Into::into).collect();
        paths.extend(Glyphs.list(path)?);
        paths.extend(Assets.list(path)?);
        Ok(paths)
    }
}
