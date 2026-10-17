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
        AlarmClock,
        AppWindow,
        Archive,
        ArchiveRestore,
        ArrowDown,
        ArrowLeft,
        ArrowUpLeft,
        ArrowUpRight,
        Book,
        Bookmark,
        Bot,
        Box,
        Brain,
        CalendarX2,
        Camera,
        CaseSensitive,
        ChartLine,
        Check,
        ChevronDown,
        ChevronLeft,
        ChevronRight,
        ChevronUp,
        ChevronsDown,
        ChevronsDownUp,
        ChevronsUpDown,
        Circle,
        CircleAlert,
        CircleArrowUp,
        CircleCheck,
        CircleDashed,
        CircleDot,
        CircleMinus,
        CircleSlash,
        CircleStop,
        ClipboardCopy,
        Clock,
        Cloud,
        CloudOff,
        Code,
        Cog,
        Computer,
        Container,
        Copy,
        CornerDownRight,
        Cpu,
        Database,
        Download,
        Earth,
        Ellipsis,
        EthernetPort,
        ExternalLink,
        Eye,
        File,
        FileDiff,
        FilePlus,
        FileText,
        FileX,
        Flame,
        FlaskConical,
        Folder,
        FolderClosed,
        Gamepad2,
        Gauge,
        GitBranch,
        GitCommitHorizontal,
        GitPullRequest,
        Github,
        Globe,
        GripVertical,
        Hammer,
        Hand,
        HardDrive,
        Heart,
        Hourglass,
        House,
        Image,
        Info,
        KeyRound,
        Keyboard,
        Laptop,
        Leaf,
        Lightbulb,
        ListChecks,
        ListTree,
        ListTodo,
        LoaderCircle,
        Lock,
        LockOpen,
        LogIn,
        Maximize2,
        MemoryStick,
        MessageCircleQuestionMark,
        MessageSquare,
        MessageSquareText,
        MessageSquareWarning,
        MessagesSquare,
        Minimize2,
        Minus,
        Monitor,
        Music,
        OctagonAlert,
        Package,
        Palette,
        PanelLeft,
        PanelLeftClose,
        PanelsTopLeft,
        Paperclip,
        PcCase,
        PenLine,
        Pencil,
        PencilRuler,
        Play,
        Plug,
        Plus,
        Power,
        PowerOff,
        Puzzle,
        Quote,
        RefreshCw,
        Regex,
        Rocket,
        RotateCcw,
        RotateCcwClock,
        RotateCw,
        Scan,
        ScrollText,
        Search,
        Server,
        Settings,
        Shield,
        SlidersHorizontal,
        Smartphone,
        Sparkles,
        Split,
        Square,
        SquarePen,
        SquareTerminal,
        SquareX,
        Star,
        Tag,
        Terminal,
        TextWrap,
        Timer,
        Trash,
        TriangleAlert,
        Undo2,
        Unplug,
        Upload,
        User,
        UserRound,
        WholeWord,
        Wrench,
        X,
        Zap,
        SquareSplitHorizontal,
        SquareSplitVertical,
        Type,
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

/// The context ring's used share at a whole percent, `0..=100`, as a glyph tinted like any other.
pub fn ring_arc(percent: u32) -> SharedString {
    format!("{RING}{}.svg", percent.min(100)).into()
}

const RING: &str = "icons/wsp-ring-";

/// The ring's arc on its 24 unit box: radius 9.75, 3 wide with round ends, clockwise from the top.
fn ring_svg(percent: u32) -> Vec<u8> {
    let share = percent.min(100) as f32 / 100.;
    let at = |radius: f32, turn: f32| {
        let a = turn * std::f32::consts::TAU;
        (12. + radius * a.sin(), 12. - radius * a.cos())
    };
    let (r, w) = (9.75f32, 3f32);
    let body = if share >= 0.999 {
        format!(r#"<circle cx="12" cy="12" r="{r}" fill="none" stroke="currentColor" stroke-width="{w}"/>"#)
    } else if share <= 0. {
        String::new()
    } else {
        let ((ox0, oy0), (ox1, oy1)) = (at(r + w / 2., 0.), at(r + w / 2., share));
        let ((ix1, iy1), (ix0, iy0)) = (at(r - w / 2., share), at(r - w / 2., 0.));
        let (large, outer, inner, cap) = (u8::from(share > 0.5), r + w / 2., r - w / 2., w / 2.);
        format!(
            r#"<path fill="currentColor" d="M{ox0} {oy0} A{outer} {outer} 0 {large} 1 {ox1} {oy1} A{cap} {cap} 0 0 1 {ix1} {iy1} A{inner} {inner} 0 {large} 0 {ix0} {iy0} A{cap} {cap} 0 0 1 {ox0} {oy0} Z"/>"#
        )
    };
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">{body}</svg>"#).into_bytes()
}

impl AssetSource for UiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == MARK {
            return Ok(Some(Cow::Borrowed(MARK_SVG)));
        }
        if path == LOCKUP {
            return Ok(Some(Cow::Borrowed(LOCKUP_SVG)));
        }
        if path == GRAIN {
            return Ok(Some(Cow::Borrowed(GRAIN_PNG)));
        }
        if let Some(percent) = path.strip_prefix(RING).and_then(|p| p.strip_suffix(".svg")).and_then(|p| p.parse().ok()) {
            return Ok(Some(Cow::Owned(ring_svg(percent))));
        }
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
            [TICK, SEND, STOP, STAR_FILLED, MARK, LOCKUP, GRAIN].into_iter().filter(|p| p.starts_with(path)).map(Into::into).collect();
        paths.extend(Glyphs.list(path)?);
        paths.extend(Assets.list(path)?);
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_alert_glyphs_are_served() {
        for glyph in [crate::ui::Glyph::Lightbulb, crate::ui::Glyph::MessageSquareWarning, crate::ui::Glyph::OctagonAlert] {
            assert!(UiAssets.load(&glyph.path()).unwrap().is_some(), "{} is not served", glyph.path());
        }
    }

    #[test]
    fn the_ring_arc_is_served_at_each_whole_percent() {
        let empty = UiAssets.load(&ring_arc(0)).unwrap().expect("an empty ring");
        assert!(!String::from_utf8_lossy(&empty).contains("<path"));
        let half = UiAssets.load(&ring_arc(50)).unwrap().expect("a half ring");
        assert!(String::from_utf8_lossy(&half).contains("A11.25 11.25 0 0 1 "), "{}", String::from_utf8_lossy(&half));
        assert!(String::from_utf8_lossy(&UiAssets.load(&ring_arc(100)).unwrap().unwrap()).contains("<circle"));
    }
}
