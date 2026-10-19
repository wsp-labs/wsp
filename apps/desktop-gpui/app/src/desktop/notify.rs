// SPDX-License-Identifier: AGPL-3.0-only
//! What the shell says over the system (`desktop:needs-you.ts`): a line said only while no window of the app has the
//! person's eyes, shown by GPUI's own notification, sounded by the system's alert beside it, or the alert alone; a
//! click raises a window, or opens one, and opens what the line was about. The words are the protocol's one rule
//! (`protocol:outside-line.ts`), so a line reads the same whichever shell says it.

use std::collections::VecDeque;
use std::rc::Rc;

use gpui_kit::{App, Global, SystemNotification};
use serde_json::Value;

use crate::screens::notices::words;

/// `OUTSIDE_HELD`: how many lines a click is held for; a click on an older one only raises the window.
pub const OUTSIDE_HELD: usize = 50;
/// `NEEDS_YOU`.
pub const NEEDS_YOU: &str = "wsp needs you";
/// The name the system files the app's notifications under.
const APP_ID: &str = "wsp.desktop.gpui";

/// One of General's choices for a kind of moment (`NOTIFY_CHOICES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyChoice {
    Off,
    Notify,
    Sound,
    NotifySound,
}

impl NotifyChoice {
    pub fn parse(word: &str) -> Option<NotifyChoice> {
        match word {
            "off" => Some(NotifyChoice::Off),
            "notify" => Some(NotifyChoice::Notify),
            "sound" => Some(NotifyChoice::Sound),
            "notify-sound" => Some(NotifyChoice::NotifySound),
            _ => None,
        }
    }

    /// `notifyBy`: whether a line under this choice shows and whether it sounds; none where it is not said at all.
    pub fn how(self) -> Option<(bool, bool)> {
        match self {
            NotifyChoice::Off => None,
            choice => Some((choice != NotifyChoice::Sound, choice != NotifyChoice::Notify)),
        }
    }
}

/// The three of General's choices a line reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choices {
    pub notify_needs: NotifyChoice,
    pub notify_done: NotifyChoice,
    pub plan_alerts: bool,
}

impl Default for Choices {
    /// `GENERAL_DEFAULTS`.
    fn default() -> Self {
        Choices { notify_needs: NotifyChoice::NotifySound, notify_done: NotifyChoice::Notify, plan_alerts: true }
    }
}

/// `OutsideLine`: one moment said outside the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutsideLine {
    pub title: String,
    pub body: String,
    pub show: bool,
    pub sound: bool,
}

/// `OutsideMoment`: the moments said outside the app, each with what its line is made of.
#[derive(Clone, Debug)]
pub enum Moment<'a> {
    Asks { line: &'a str },
    SignIn { what: &'a str },
    Finished { thread: &'a str, place: &'a str },
    Failed { thread: &'a str, place: &'a str, error: Option<&'a str> },
    Awake { workspace: &'a str },
    Plan { label: &'a str, alert: &'a Value },
    SetupFailed { computer: &'a str, said: &'a str },
    SetupNeedsYou { computer: &'a str, what: &'a str },
    SetupReady { computer: &'a str, missed: Option<&'a str> },
    ImageReady { computer: &'a str, image: &'a str },
}

/// `oneLine`: every run of white space one space, trimmed.
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `setupFailedLine`.
pub fn setup_failed(computer: &str) -> String {
    format!("{computer}: setup failed")
}

/// `setupReadyLine`, with what the setup missed where it missed something.
pub fn setup_ready(computer: &str, missed: Option<&str>) -> String {
    match missed {
        Some(missed) => format!("{computer} is ready; {missed}"),
        None => words::setup_ready(computer),
    }
}

/// `outsideLine`: the line a moment makes under the person's choices, or none where they chose not to hear it.
pub fn outside_line(moment: &Moment, choices: &Choices) -> Option<OutsideLine> {
    let said = |how: Option<(bool, bool)>, title: String, body: String| how.map(|(show, sound)| OutsideLine { title, body, show, sound });
    let needs = choices.notify_needs.how();
    let done = choices.notify_done.how();
    match *moment {
        Moment::Asks { line } => said(needs, NEEDS_YOU.to_owned(), line.to_owned()),
        Moment::SignIn { what } => said(needs, NEEDS_YOU.to_owned(), what.to_owned()),
        Moment::Finished { thread, place } => said(done, format!("{thread} finished"), place.to_owned()),
        Moment::Failed { thread, place, error } => {
            said(done, format!("{thread} stopped"), error.map_or_else(|| place.to_owned(), one_line))
        }
        Moment::Awake { workspace } => said(done, NEEDS_YOU.to_owned(), format!("{workspace} is awake")),
        Moment::Plan { label, alert } => said(choices.plan_alerts.then_some((true, false)), words::plan_alert(label, alert), String::new()),
        Moment::SetupFailed { computer, said: line } => said(needs, setup_failed(computer), one_line(line)),
        Moment::SetupNeedsYou { computer, what } => said(needs, NEEDS_YOU.to_owned(), words::setup_needs_you(computer, what)),
        Moment::SetupReady { computer, missed } => said(done, setup_ready(computer, missed), String::new()),
        Moment::ImageReady { computer, image } => said(done, setup_ready(computer, None), image.to_owned()),
    }
}

/// What saying a line comes to (`sayOutside`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saying {
    /// A window has the person's eyes, the line neither shows nor sounds, or this computer shows no notifications.
    Nothing,
    /// The system's alert alone, for a line the person chose to hear and not see.
    Beep,
    /// A notification, with the alert beside it where the line sounds.
    Show { sound: bool },
}

pub fn saying(line: &OutsideLine, focused: bool, supported: bool) -> Saying {
    if focused || (!line.show && !line.sound) {
        return Saying::Nothing;
    }
    if !line.show {
        return Saying::Beep;
    }
    if !supported {
        return Saying::Nothing;
    }
    Saying::Show { sound: line.sound }
}

/// The lines held for their clicks by tag, oldest first; past [`OUTSIDE_HELD`] the oldest is let go.
pub struct Held<T> {
    lines: VecDeque<(String, T)>,
}

impl<T> Default for Held<T> {
    fn default() -> Self {
        Held { lines: VecDeque::new() }
    }
}

impl<T> Held<T> {
    pub fn hold(&mut self, tag: String, what: T) {
        self.lines.push_back((tag, what));
        if self.lines.len() > OUTSIDE_HELD {
            self.lines.pop_front();
        }
    }

    /// The line a click landed on, let go; none for a line no longer held.
    pub fn take(&mut self, tag: &str) -> Option<T> {
        let at = self.lines.iter().position(|(held, _)| held == tag)?;
        self.lines.remove(at).map(|(_, what)| what)
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// What a click on a line opens, run once the window stands.
pub type Opener = Rc<dyn Fn(&mut App)>;

/// The road out of the app: the lines held for their clicks and the window a click brings back.
struct Road {
    held: Held<Option<Opener>>,
    said: u64,
    reopen: Rc<dyn Fn(&mut App)>,
    /// Whether the system takes a line from this app at all: a Mac app outside a bundle has no identity to post under.
    bundled: bool,
    refused: u64,
}

impl Global for Road {}

/// Names the app to the system and routes a click on a line to what it was about. `reopen` opens the app's window
/// where none stands.
pub fn init(reopen: Rc<dyn Fn(&mut App)>, cx: &mut App) {
    cx.set_app_identity(APP_ID, crate::TITLE);
    let bundled = cfg!(not(target_os = "macos")) || bundle_here();
    cx.set_global(Road { held: Held::default(), said: 0, reopen, bundled, refused: 0 });
    cx.on_system_notification_response(|response, cx| {
        let open = if cx.has_global::<Road>() { cx.global_mut::<Road>().held.take(&response.tag).flatten() } else { None };
        raise(cx);
        if let Some(open) = open {
            open(cx);
        }
    });
}

#[cfg(target_os = "macos")]
fn bundle_here() -> bool {
    super::mac::bundled()
}

#[cfg(not(target_os = "macos"))]
fn bundle_here() -> bool {
    true
}

/// Takes the app as running from a bundle or not, whatever it was launched from: a test reaches both roads with it.
pub fn take_as_bundled(bundled: bool, cx: &mut App) {
    cx.global_mut::<Road>().bundled = bundled;
}

/// How many lines the system was not handed, each logged with its reason.
pub fn refused_count(cx: &App) -> u64 {
    cx.try_global::<Road>().map_or(0, |road| road.refused)
}

/// Brings the app's window in front of the person, or opens it again where none stands.
pub fn raise(cx: &mut App) {
    match cx.windows().last().copied() {
        Some(window) => _ = window.update(cx, |_, window, cx| super::frame::raise(window, cx)),
        None => {
            cx.activate(true);
            if let Some(reopen) = cx.try_global::<Road>().map(|r| r.reopen.clone()) {
                reopen(cx);
            }
        }
    }
}

/// Whether a window of the app has the person's eyes.
pub fn focused(cx: &mut App) -> bool {
    cx.windows().into_iter().any(|window| window.update(cx, |_, window, _| window.is_window_active()).unwrap_or(false))
}

/// The system's alert sound.
pub fn beep(cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        _ = &cx;
        super::mac::beep();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = cx.active_window().or_else(|| cx.windows().last().copied()) {
        _ = window.update(cx, |_, window, _| window.play_system_bell());
    }
}

/// A line the system would not show (`NOTIFIER.refused`): said in the log, and the person still drawn to the app.
fn refused(error: &str, sound: bool, cx: &mut App) {
    eprintln!("desktop-gpui: notification not shown: {error}");
    if cx.has_global::<Road>() {
        cx.global_mut::<Road>().refused += 1;
    }
    #[cfg(target_os = "macos")]
    super::mac::bounce();
    if sound {
        beep(cx);
    }
}

/// The body as text: a freedesktop server reads the body as markup, and a prompt's body is the tool input an agent
/// wrote, which must not draw a link the person can click. A Mac's notification takes plain text.
fn as_text(body: &str) -> String {
    if cfg!(target_os = "linux") {
        body.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    } else {
        body.to_owned()
    }
}

/// Says `line` outside the app, or nothing while a window has the person's eyes; a click on it raises the window and
/// runs `open`. True where anything was said.
pub fn say(line: &OutsideLine, open: Option<Opener>, cx: &mut App) -> bool {
    let focused = focused(cx);
    match saying(line, focused, true) {
        Saying::Nothing => false,
        Saying::Beep => {
            beep(cx);
            true
        }
        Saying::Show { sound } => {
            if !cx.has_global::<Road>() {
                refused("the app's notifications are not set up", sound, cx);
                return true;
            }
            if !cx.global::<Road>().bundled {
                refused("not running from an app bundle", sound, cx);
                return true;
            }
            let road = cx.global_mut::<Road>();
            road.said += 1;
            let tag = format!("wsp-{}", road.said);
            road.held.hold(tag.clone(), open);
            cx.show_system_notification(SystemNotification {
                tag: tag.into(),
                title: line.title.clone().into(),
                body: as_text(&line.body).into(),
                actions: Vec::new(),
            });
            if sound {
                beep(cx);
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NEED: fn() -> OutsideLine =
        || OutsideLine { title: NEEDS_YOU.to_owned(), body: "sign in to GitHub CLI login".to_owned(), show: true, sound: false };

    #[test]
    fn each_choice_shows_and_sounds_as_general_says() {
        assert_eq!(NotifyChoice::Off.how(), None);
        assert_eq!(NotifyChoice::Notify.how(), Some((true, false)));
        assert_eq!(NotifyChoice::Sound.how(), Some((false, true)));
        assert_eq!(NotifyChoice::NotifySound.how(), Some((true, true)));
        assert_eq!(NotifyChoice::parse("notify-sound"), Some(NotifyChoice::NotifySound));
        assert_eq!(NotifyChoice::parse("loud"), None);
    }

    #[test]
    fn a_long_title_is_handed_over_whole() {
        let title = "x".repeat(200);
        let line = outside_line(&Moment::Finished { thread: &title, place: "zmac" }, &Choices::default()).unwrap();
        assert_eq!(line.title, format!("{title} finished"));
    }

    #[test]
    fn each_moment_says_the_protocols_words() {
        let loud = Choices { notify_needs: NotifyChoice::NotifySound, notify_done: NotifyChoice::NotifySound, plan_alerts: true };
        let line = |m: Moment| outside_line(&m, &loud).map(|l| (l.title, l.body));
        let pair = |t: &str, b: &str| Some((t.to_owned(), b.to_owned()));
        assert_eq!(line(Moment::Asks { line: "Run a command: ls" }), pair("wsp needs you", "Run a command: ls"));
        assert_eq!(line(Moment::SignIn { what: "spoo: sign in" }), pair("wsp needs you", "spoo: sign in"));
        assert_eq!(line(Moment::Finished { thread: "fix login", place: "zmac" }), pair("fix login finished", "zmac"));
        assert_eq!(
            line(Moment::Failed { thread: "fix login", place: "zmac", error: Some("API Error: 529\n overloaded") }),
            pair("fix login stopped", "API Error: 529 overloaded")
        );
        assert_eq!(line(Moment::Failed { thread: "fix login", place: "zmac", error: None }), pair("fix login stopped", "zmac"));
        assert_eq!(line(Moment::Awake { workspace: "api" }), pair("wsp needs you", "api is awake"));
        assert_eq!(
            line(Moment::Plan { label: "Max plan", alert: &json!({ "kind": "low", "window": "week", "step": 90 }) }),
            pair("Max plan has used 90% of its week", "")
        );
        assert_eq!(line(Moment::SetupFailed { computer: "spoo", said: "no\nroute" }), pair("spoo: setup failed", "no route"));
        assert_eq!(line(Moment::SetupNeedsYou { computer: "spoo", what: "a code" }), pair("wsp needs you", "spoo needs you: a code"));
        assert_eq!(line(Moment::SetupReady { computer: "spoo", missed: Some("no GitHub") }), pair("spoo is ready; no GitHub", ""));
        assert_eq!(line(Moment::SetupReady { computer: "spoo", missed: None }), pair("spoo is ready", ""));
        assert_eq!(line(Moment::ImageReady { computer: "spoo", image: "v3" }), pair("spoo is ready", "v3"));
    }

    #[test]
    fn a_plan_alert_shows_with_no_sound_while_its_switch_is_on() {
        let alert = json!({ "kind": "blocked" });
        let on = outside_line(&Moment::Plan { label: "Max plan", alert: &alert }, &Choices::default()).unwrap();
        assert!(on.show && !on.sound);
        let off = Choices { plan_alerts: false, ..Choices::default() };
        assert_eq!(outside_line(&Moment::Plan { label: "Max plan", alert: &alert }, &off), None);
    }

    /// `needs-you.test.ts`: shown with no sound once the window lost focus; nothing while it has it.
    #[test]
    fn a_focused_window_says_nothing_and_an_unfocused_one_shows_the_line() {
        assert_eq!(saying(&NEED(), false, true), Saying::Show { sound: false });
        assert_eq!(saying(&NEED(), true, true), Saying::Nothing);
    }

    /// `needs-you.test.ts`: a line to hear and not see plays the alert alone, and none while the window has focus.
    #[test]
    fn a_sound_only_line_plays_the_alert_and_shows_nothing() {
        let sound = OutsideLine { show: false, sound: true, ..NEED() };
        assert_eq!(saying(&sound, false, true), Saying::Beep);
        assert_eq!(saying(&sound, true, true), Saying::Nothing);
        assert_eq!(saying(&OutsideLine { show: false, sound: false, ..NEED() }, false, true), Saying::Nothing);
    }

    /// `needs-you.test.ts`: a computer that shows no notifications builds none.
    #[test]
    fn a_computer_with_no_notifications_is_told_nothing() {
        assert_eq!(saying(&NEED(), false, false), Saying::Nothing);
    }

    /// `needs-you.test.ts`: a line that asks for a sound makes one, and one that does not stays silent.
    #[test]
    fn a_line_that_sounds_shows_with_the_alert() {
        assert_eq!(saying(&OutsideLine { sound: true, ..NEED() }, false, true), Saying::Show { sound: true });
    }

    #[cfg(target_os = "linux")]
    #[gpui_kit::test]
    fn an_agents_markup_in_a_body_reaches_the_server_as_text(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| init(Rc::new(|_| {}), cx));
        let body = r#"echo '<a href="https://example.invalid">docs</a>' && ls"#;
        assert!(cx.update(|cx| say(&OutsideLine { body: body.to_owned(), ..NEED() }, None, cx)));
        let shown = cx.shown_system_notifications();
        assert_eq!(shown[0].body.as_ref(), r#"echo '&lt;a href="https://example.invalid"&gt;docs&lt;/a&gt;' &amp;&amp; ls"#);
    }

    /// `needs-you.test.ts`: fifty lines are held for their clicks, then the oldest goes.
    #[test]
    fn fifty_lines_are_held_then_the_oldest_goes() {
        let mut held = Held::default();
        for n in 0..OUTSIDE_HELD + 1 {
            held.hold(format!("wsp-{n}"), n);
        }
        assert_eq!(held.len(), OUTSIDE_HELD);
        assert_eq!(held.take("wsp-0"), None);
        assert_eq!(held.take("wsp-1"), Some(1));
        assert_eq!(held.take("wsp-1"), None, "a click takes its line");
        assert_eq!(held.take(&format!("wsp-{OUTSIDE_HELD}")), Some(OUTSIDE_HELD));
    }
}
