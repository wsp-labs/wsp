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
        ArrowDown,
        ArrowLeft,
        ArrowRight,
        ArrowUp,
        ArrowUpToLine,
        Book,
        BookOpen,
        Bookmark,
        Bot,
        Box,
        Brain,
        Bug,
        CalendarX2,
        Camera,
        ChartLine,
        Check,
        ChevronDown,
        ChevronLeft,
        ChevronRight,
        ChevronUp,
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
        CircleX,
        ClipboardCopy,
        Clock,
        Cloud,
        CloudOff,
        Code,
        CodeXml,
        Cog,
        Columns2,
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
        FileText,
        FileX,
        Flame,
        FlaskConical,
        Folder,
        FolderClosed,
        FolderGit,
        FolderOpen,
        FolderOutput,
        FolderPlus,
        Gamepad2,
        Gauge,
        GitBranch,
        GitCommitHorizontal,
        GitFork,
        GitMerge,
        GitPullRequest,
        GitPullRequestArrow,
        GitPullRequestClosed,
        GitPullRequestDraft,
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
        ListChecks,
        ListTodo,
        LoaderCircle,
        Lock,
        LockOpen,
        LogIn,
        Maximize2,
        MessageCircle,
        MessageCircleQuestionMark,
        MessageSquare,
        MessageSquarePlus,
        MessageSquareText,
        MessagesSquare,
        MemoryStick,
        Minus,
        Monitor,
        Music,
        Package,
        Palette,
        PanelLeft,
        PanelLeftClose,
        PanelRight,
        PanelsTopLeft,
        Paperclip,
        Pause,
        PcCase,
        PenLine,
        Pencil,
        PencilRuler,
        Pin,
        PinOff,
        Play,
        Plug,
        Plus,
        Power,
        PowerOff,
        Puzzle,
        Quote,
        Receipt,
        RefreshCw,
        Rocket,
        RotateCcw,
        RotateCcwClock,
        RotateCw,
        Scan,
        Rows3,
        ScanEye,
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
        SquareArrowOutUpRight,
        SquarePen,
        SquareTerminal,
        SquareX,
        Star,
        Tag,
        Terminal,
        TextSearch,
        Trash,
        TriangleAlert,
        Unplug,
        Upload,
        User,
        UserRound,
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
/// lucide's smile-plus, which gpui-kit's bundle lacks: a reaction's add button.
pub const SMILE_PLUS: &str = "icons/wsp-smile-plus.svg";
const SMILE_PLUS_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11v1a10 10 0 1 1-9-10"/><path d="M8 14s1.5 2 4 2 4-2 4-2"/><line x1="9" x2="9.01" y1="9" y2="9"/><line x1="15" x2="15.01" y1="9" y2="9"/><path d="M16 5h6"/><path d="M19 2v6"/></svg>"#;
const STOP_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="currentColor"><rect x="2" y="2" width="8" height="8" rx="1.5"/></svg>"#;
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

/// A diff file head's kind marks and its rename arrow, pierre's own (`@pierre/diffs` sprite), since lucide has none of them.
pub const DIFF_ADDED: &str = "icons/wsp-diff-added.svg";
const DIFF_ADDED_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path d="M8 4a.75.75 0 0 1 .75.75v2.5h2.5a.75.75 0 0 1 0 1.5h-2.5v2.5a.75.75 0 0 1-1.5 0v-2.5h-2.5a.75.75 0 0 1 0-1.5h2.5v-2.5A.75.75 0 0 1 8 4"/><path d="M1.788 4.296c.196-.88.478-1.381.802-1.706s.826-.606 1.706-.802C5.194 1.588 6.387 1.5 8 1.5s2.806.088 3.704.288c.88.196 1.381.478 1.706.802s.607.826.802 1.706c.2.898.288 2.091.288 3.704s-.088 2.806-.288 3.704c-.195.88-.478 1.381-.802 1.706s-.826.607-1.706.802c-.898.2-2.091.288-3.704.288s-2.806-.088-3.704-.288c-.88-.195-1.381-.478-1.706-.802s-.606-.826-.802-1.706C1.588 10.806 1.5 9.613 1.5 8s.088-2.806.288-3.704M8 0C1.412 0 0 1.412 0 8s1.412 8 8 8 8-1.412 8-8-1.412-8-8-8"/></svg>"#;
pub const DIFF_DELETED: &str = "icons/wsp-diff-deleted.svg";
const DIFF_DELETED_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path d="M4 8a.75.75 0 0 1 .75-.75h6.5a.75.75 0 0 1 0 1.5h-6.5A.75.75 0 0 1 4 8"/><path d="M1.788 4.296c.196-.88.478-1.381.802-1.706s.826-.606 1.706-.802C5.194 1.588 6.387 1.5 8 1.5s2.806.088 3.704.288c.88.196 1.381.478 1.706.802s.607.826.802 1.706c.2.898.288 2.091.288 3.704s-.088 2.806-.288 3.704c-.195.88-.478 1.381-.802 1.706s-.826.607-1.706.802c-.898.2-2.091.288-3.704.288s-2.806-.088-3.704-.288c-.88-.195-1.381-.478-1.706-.802s-.606-.826-.802-1.706C1.588 10.806 1.5 9.613 1.5 8s.088-2.806.288-3.704M8 0C1.412 0 0 1.412 0 8s1.412 8 8 8 8-1.412 8-8-1.412-8-8-8"/></svg>"#;
pub const DIFF_MODIFIED: &str = "icons/wsp-diff-modified.svg";
const DIFF_MODIFIED_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path d="M1.5 8c0 1.613.088 2.806.288 3.704.196.88.478 1.381.802 1.706s.826.607 1.706.802c.898.2 2.091.288 3.704.288s2.806-.088 3.704-.288c.88-.195 1.381-.478 1.706-.802s.607-.826.802-1.706c.2-.898.288-2.091.288-3.704s-.088-2.806-.288-3.704c-.195-.88-.478-1.381-.802-1.706s-.826-.606-1.706-.802C10.806 1.588 9.613 1.5 8 1.5s-2.806.088-3.704.288c-.88.196-1.381.478-1.706.802s-.606.826-.802 1.706C1.588 5.194 1.5 6.387 1.5 8M0 8c0-6.588 1.412-8 8-8s8 1.412 8 8-1.412 8-8 8-8-1.412-8-8m8 3a3 3 0 1 0 0-6 3 3 0 0 0 0 6"/></svg>"#;
pub const DIFF_MOVED: &str = "icons/wsp-diff-moved.svg";
const DIFF_MOVED_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path d="M1.788 4.296c.196-.88.478-1.381.802-1.706s.826-.606 1.706-.802C5.194 1.588 6.387 1.5 8 1.5s2.806.088 3.704.288c.88.196 1.381.478 1.706.802s.607.826.802 1.706c.2.898.288 2.091.288 3.704s-.088 2.806-.288 3.704c-.195.88-.478 1.381-.802 1.706s-.826.607-1.706.802c-.898.2-2.091.288-3.704.288s-2.806-.088-3.704-.288c-.88-.195-1.381-.478-1.706-.802s-.606-.826-.802-1.706C1.588 10.806 1.5 9.613 1.5 8s.088-2.806.288-3.704M8 0C1.412 0 0 1.412 0 8s1.412 8 8 8 8-1.412 8-8-1.412-8-8-8"/><path d="M8.495 4.695a.75.75 0 0 0-.05 1.06L10.486 8l-2.041 2.246a.75.75 0 0 0 1.11 1.008l2.5-2.75a.75.75 0 0 0 0-1.008l-2.5-2.75a.75.75 0 0 0-1.06-.051m-4 0a.75.75 0 0 0-.05 1.06l2.044 2.248-1.796 1.995a.75.75 0 0 0 1.114 1.004l2.25-2.5a.75.75 0 0 0-.002-1.007l-2.5-2.75a.75.75 0 0 0-1.06-.05"/></svg>"#;
pub const DIFF_RENAMED_TO: &str = "icons/wsp-diff-renamed-to.svg";
const DIFF_RENAMED_TO_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path d="M8.47 4.22a.75.75 0 0 0 0 1.06l1.97 1.97H3.75a.75.75 0 0 0 0 1.5h6.69l-1.97 1.97a.75.75 0 1 0 1.06 1.06l3.25-3.25a.75.75 0 0 0 0-1.06L9.53 4.22a.75.75 0 0 0-1.06 0"/></svg>"#;

/// The glyphs a piece names, by lucide's own names.
pub type Glyph = IconName;

/// gpui-kit's bundle with the pieces' glyphs over it; open the application with it.
#[derive(Clone, Copy, Debug, Default)]
pub struct UiAssets;

impl AssetSource for UiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        for (own, svg) in [
            (TICK, TICK_SVG),
            (SEND, SEND_SVG),
            (STOP, STOP_SVG),
            (STAR_FILLED, STAR_FILLED_SVG),
            (SMILE_PLUS, SMILE_PLUS_SVG),
            (DIFF_ADDED, DIFF_ADDED_SVG),
            (DIFF_DELETED, DIFF_DELETED_SVG),
            (DIFF_MODIFIED, DIFF_MODIFIED_SVG),
            (DIFF_MOVED, DIFF_MOVED_SVG),
            (DIFF_RENAMED_TO, DIFF_RENAMED_TO_SVG),
        ] {
            if path == own {
                return Ok(Some(Cow::Borrowed(svg)));
            }
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
        let mut paths: Vec<SharedString> = [
            TICK,
            SEND,
            STOP,
            STAR_FILLED,
            SMILE_PLUS,
            DIFF_ADDED,
            DIFF_DELETED,
            DIFF_MODIFIED,
            DIFF_MOVED,
            DIFF_RENAMED_TO,
            MARK,
            LOCKUP,
            GRAIN,
        ]
        .into_iter()
        .filter(|p| p.starts_with(path))
        .map(Into::into)
        .collect();
        paths.extend(Glyphs.list(path)?);
        paths.extend(Assets.list(path)?);
        Ok(paths)
    }
}
