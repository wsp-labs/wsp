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
        Archive,
        Bot,
        CalendarX2,
        Check,
        ChevronDown,
        ChevronRight,
        Circle,
        CircleAlert,
        CircleCheck,
        CircleMinus,
        CircleStop,
        Cloud,
        CloudOff,
        Copy,
        CornerDownRight,
        Folder,
        Gauge,
        GitPullRequest,
        Hourglass,
        KeyRound,
        LoaderCircle,
        MessageCircleQuestionMark,
        Monitor,
        PanelLeft,
        PanelLeftClose,
        Plus,
        RotateCcw,
        Search,
        Settings,
        Square,
        SquarePen,
        Terminal,
        Upload,
        X,
    ]
);

/// The checkbox's tick, which is not lucide's: a 3 px stroke on its own path.
pub const TICK: &str = "icons/wsp-tick.svg";
const TICK_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5.252 12.7 10.2 18.63 18.748 5.37"/></svg>"#;

/// The tilde beside the wordmark, `Lockup` in `apps/web/src/brand/Brand.tsx`, drawn as a mask in the ink it is given.
pub const LOCKUP: &str = "icons/wsp-lockup.svg";
const LOCKUP_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 51 14" fill="none" stroke="#000" stroke-linecap="round" stroke-linejoin="round"><g transform="translate(0 -1.08) scale(0.76)" stroke-width="3"><path d="M1.6 8A3.4 3.4 0 0 1 8 8A3.4 3.4 0 0 0 14.4 8"/></g><g transform="translate(17 0)" stroke-width="2"><path d="M1 1L3.5 9L6 3L8.5 9L11 1"/><path d="M20.9 3A2.4 2 0 1 0 18.5 5A2.4 2 0 1 1 16.1 7"/><path d="M26 1V13M26 1H29A4 4 0 0 1 29 9H26"/></g></svg>"##;

/// An agent's mark as its catalog module carries it (`packages/catalog/src/agents/`): the svg, drawn as a mask, and
/// its ink on each side. An agent with no row here is drawn as its initials.
pub struct AgentMark {
    pub agent: &'static str,
    pub path: &'static str,
    svg: &'static [u8],
    pub light: &'static str,
    pub dark: &'static str,
}

pub const AGENT_MARKS: [AgentMark; 1] = [AgentMark {
    agent: "claude",
    path: "icons/wsp-agent-claude.svg",
    svg: br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 257"><path d="m50.228 170.321 50.357-28.257.843-2.463-.843-1.361h-2.462l-8.426-.518-28.775-.778-24.952-1.037-24.175-1.296-6.092-1.297L0 125.796l.583-3.759 5.12-3.434 7.324.648 16.202 1.101 24.304 1.685 17.629 1.037 26.118 2.722h4.148l.583-1.685-1.426-1.037-1.101-1.037-25.147-17.045-27.22-18.017-14.258-10.37-7.713-5.25-3.888-4.925-1.685-10.758 7-7.713 9.397.649 2.398.648 9.527 7.323 20.35 15.75L94.817 91.9l3.889 3.24 1.555-1.102.195-.777-1.75-2.917-14.453-26.118-15.425-26.572-6.87-11.018-1.814-6.61c-.648-2.723-1.102-4.991-1.102-7.778l7.972-10.823L71.42 0 82.05 1.426l4.472 3.888 6.61 15.101 10.694 23.786 16.591 32.34 4.861 9.592 2.592 8.879.973 2.722h1.685v-1.556l1.36-18.211 2.528-22.36 2.463-28.776.843-8.1 4.018-9.722 7.971-5.25 6.222 2.981 5.12 7.324-.713 4.73-3.046 19.768-5.962 30.98-3.889 20.739h2.268l2.593-2.593 10.499-13.934 17.628-22.036 7.778-8.749 9.073-9.657 5.833-4.601h11.018l8.1 12.055-3.628 12.443-11.342 14.388-9.398 12.184-13.48 18.147-8.426 14.518.778 1.166 2.01-.194 30.46-6.481 16.462-2.982 19.637-3.37 8.88 4.148.971 4.213-3.5 8.62-20.998 5.184-24.628 4.926-36.682 8.685-.454.324.519.648 16.526 1.555 7.065.389h17.304l32.21 2.398 8.426 5.574 5.055 6.805-.843 5.184-12.962 6.611-17.498-4.148-40.83-9.721-14-3.5h-1.944v1.167l11.666 11.406 21.387 19.314 26.767 24.887 1.36 6.157-3.434 4.86-3.63-.518-23.526-17.693-9.073-7.972-20.545-17.304h-1.36v1.814l4.73 6.935 25.017 37.59 1.296 11.536-1.814 3.76-6.481 2.268-7.13-1.297-14.647-20.544-15.1-23.138-12.185-20.739-1.49.843-7.194 77.448-3.37 3.953-7.778 2.981-6.48-4.925-3.436-7.972 3.435-15.749 4.148-20.544 3.37-16.333 3.046-20.285 1.815-6.74-.13-.454-1.49.194-15.295 20.999-23.267 31.433-18.406 19.702-4.407 1.75-7.648-3.954.713-7.064 4.277-6.286 25.47-32.405 15.36-20.092 9.917-11.6-.065-1.686h-.583L44.07 198.125l-12.055 1.555-5.185-4.86.648-7.972 2.463-2.593 20.35-13.999-.064.065Z"/></svg>"#,
    light: "#c4643f",
    dark: "#d97757",
}];

pub fn agent_mark(agent: &str) -> Option<&'static AgentMark> {
    AGENT_MARKS.iter().find(|m| m.agent == agent)
}

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
        if path == LOCKUP {
            return Ok(Some(Cow::Borrowed(LOCKUP_SVG)));
        }
        if let Some(mark) = AGENT_MARKS.iter().find(|m| m.path == path) {
            return Ok(Some(Cow::Borrowed(mark.svg)));
        }
        match Glyphs.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> =
            [TICK, LOCKUP].into_iter().chain(AGENT_MARKS.iter().map(|m| m.path)).filter(|p| p.starts_with(path)).map(Into::into).collect();
        paths.extend(Glyphs.list(path)?);
        paths.extend(Assets.list(path)?);
        Ok(paths)
    }
}
