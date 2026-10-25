// SPDX-License-Identifier: AGPL-3.0-only
//! `SignInTerminal`: a sign-in that asks the person to pick, drawn in place as the terminal it runs in on that
//! computer. The host started the agent's own login in a pty there, and this attaches to it over the channel to that
//! computer (`daemon.open`), held while the flow stands and let go with it. The host watches the pty end and reads
//! whether it signed in.

use gpui_kit::{
    div, prelude::*, px, App, Context, Entity, Focusable as _, Global, KeyBinding, NoAction, SharedString, Subscription, Task, Window,
};

use crate::screens::terminal::link::{Link, LinkEvent, LinkHold, Status, Target};
use crate::screens::terminal::look::{Choices, Look};
use crate::screens::terminal::session::Session;
use crate::screens::terminal::view::TerminalView;
use crate::screens::Host;
use crate::ui::{size, ActivePalette};

/// About 17 rows: OpenCode's provider picker draws 15 with its question, and lists past seven providers scroll inside it.
const BOX: f32 = 344.;
/// Escape is the login's own (OpenCode's picker quits on it), never Settings' or the page's around it.
const CONTEXT: &str = "SignInTerminal";

#[derive(Default)]
struct EscapeBound(bool);

impl Global for EscapeBound {}

pub struct SignInTerminal {
    link: Entity<Link>,
    hold: Option<LinkHold>,
    pty: SharedString,
    view: Option<Entity<TerminalView>>,
    /// A resume asked on this live channel, asked again only once the channel goes live again.
    asked: bool,
    resuming: Option<Task<()>>,
    _changed: Subscription,
}

impl SignInTerminal {
    pub fn new(host: &Host, place: &str, pty: &str, window: &mut Window, cx: &mut Context<Self>) -> SignInTerminal {
        if !std::mem::replace(&mut cx.default_global::<EscapeBound>().0, true) {
            cx.bind_keys([KeyBinding::new("escape", NoAction, Some(CONTEXT))]);
        }
        let link = Link::get(host, Target::Place(place.to_owned()), cx);
        let hold = link.update(cx, |link, _| link.hold());
        let changed = cx.subscribe_in(&link, window, |this, _, _: &LinkEvent, window, cx| this.attach(window, cx));
        // Another sign-in on the same computer may still hold the channel, which then goes with the last of them.
        cx.on_release(|this: &mut SignInTerminal, cx| {
            this.hold = None;
            this.link.update(cx, |link, cx| link.let_go_unheld(cx));
        })
        .detach();
        let mut this = SignInTerminal {
            link,
            hold: Some(hold),
            pty: pty.to_owned().into(),
            view: None,
            asked: false,
            resuming: None,
            _changed: changed,
        };
        this.attach(window, cx);
        this
    }

    /// The login's shell once the channel is live: the one the channel adopted, else attached by its id.
    fn attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view.is_some() {
            return;
        }
        let link = self.link.read(cx);
        if *link.status() != Status::Live {
            self.asked = false;
            return;
        }
        let adopted = link.ptys().iter().find(|p| *p.session.read(cx).pty() == self.pty).map(|p| p.session.clone());
        if let Some(session) = adopted {
            self.show(session, window, cx);
            return;
        }
        if std::mem::replace(&mut self.asked, true) {
            return;
        }
        let resumed = self.link.update(cx, |link, cx| link.resume_run(&self.pty, None, cx));
        self.resuming = Some(cx.spawn_in(window, async move |this, cx| {
            if !resumed.await {
                return;
            }
            _ = this.update_in(cx, |this, window, cx| {
                let run = this.link.read(cx).reply_pty(&this.pty, cx);
                if let Some(session) = run.map(|run| run.read(cx).session().clone()) {
                    this.show(session, window, cx);
                }
            });
        }));
    }

    fn show(&mut self, session: Entity<Session>, window: &mut Window, cx: &mut Context<Self>) {
        if self.view.is_some() {
            return;
        }
        let look = Look::with(None, &Choices::default(), cx);
        let view = cx.new(|cx| TerminalView::new(session, look, window, cx));
        view.focus_handle(cx).focus(window, cx);
        self.view = Some(view);
        cx.notify();
    }
}

impl Render for SignInTerminal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        framed(cx).children(self.view.clone())
    }
}

/// The box the terminal draws in, which holds Escape for it (`data-owns-keys`).
fn framed(cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = cx.palette();
    div()
        .id("sign-in-terminal")
        .debug_selector(|| "sign-in-terminal".into())
        .key_context(CONTEXT)
        .h(px(BOX))
        .overflow_hidden()
        .rounded(size::CONTROL_RADIUS)
        .border_1()
        .border_color(p.border)
        .bg(p.terminal_background)
}

/// The terminal of the sign-in running in `pty` on `place`, kept while it is drawn; an empty box with no host.
pub fn view(host: Option<Host>, key: SharedString, place: &str, pty: &str, window: &mut Window, cx: &mut App) -> gpui_kit::AnyElement {
    let Some(host) = host else { return framed(cx).into_any_element() };
    let (place, pty) = (place.to_owned(), pty.to_owned());
    window
        .use_keyed_state(SharedString::from(format!("{key}-{pty}")), cx, |window, cx| SignInTerminal::new(&host, &place, &pty, window, cx))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gpui_kit::{Entity, KeyDownEvent, TestAppContext, VisualTestContext};
    use serde_json::{json, Value};

    use super::super::flow;
    use super::super::store::{agent_row, Start, Store};
    use super::*;
    use crate::screens::app_shell::keys::{EscapePage, SETTINGS_OPEN, SHELL};
    use crate::screens::settings::door::{runtime, Door};
    use crate::screens::settings::Acts;

    const PLACE: &str = "box1";

    /// Two rows' sign-ins on `PLACE` drawn under a root that closes Settings on Escape, by the shell's binding and by
    /// Settings' own key handler, each counted.
    struct Shown {
        store: Entity<Store>,
        escapes: Rc<Cell<usize>>,
    }

    impl Render for Shown {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let rows = [agent_row("opencode"), agent_row("crush")];
            let flows: Vec<_> = rows
                .iter()
                .filter_map(|row| Some((row, self.store.read(cx).flows.get(&(PLACE.to_owned(), row.clone()))?.clone())))
                .collect();
            let (action, key) = (self.escapes.clone(), self.escapes.clone());
            div()
                .id("settings-root")
                .key_context(format!("{SHELL} {SETTINGS_OPEN}").as_str())
                .on_action(move |_: &EscapePage, _, _| action.set(action.get() + 1))
                .on_key_down(move |e: &KeyDownEvent, _, _| {
                    if e.keystroke.key == "escape" {
                        key.set(key.get() + 1);
                    }
                })
                .w(px(760.))
                .children(flows.into_iter().map(|(row, f)| flow::view(&self.store, PLACE, row, &f, true, window, cx)))
        }
    }

    fn waits(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            assert!(Instant::now() < deadline, "never came to: {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn daemon_frames(door: &Door, op: &str) -> Vec<Value> {
        door.asked_of("daemon.send").into_iter().map(|f| f["frame"].clone()).filter(|f| f["op"] == op).collect()
    }

    /// Catches: a login that asks the person to pick, on a computer the person added, not asked of the host as a
    /// terminal sign-in, not drawn as a terminal attached over that computer's channel, or Escape in it going to Settings
    /// (the shell's binding or Settings' own key handler) rather than to the login, or the computer's channel left up
    /// once the last sign-in there and its terminal are gone, or closed under another still drawn.
    #[gpui_kit::test]
    fn a_terminal_sign_in_opens_in_place_and_escape_goes_to_the_login(cx: &mut TestAppContext) {
        signs_in_in_place(json!([{ "id": "p1", "cols": 80, "rows": 17 }]), cx);
    }

    /// Catches: a login's shell the channel did not adopt as it went live (one started after) never attached.
    #[gpui_kit::test]
    fn a_login_started_after_the_channel_went_live_is_attached_by_its_id(cx: &mut TestAppContext) {
        signs_in_in_place(json!([]), cx);
    }

    /// The sign-in pressed on `PLACE`, whose daemon lists `listed` as its channel goes live.
    fn signs_in_in_place(listed: Value, cx: &mut TestAppContext) {
        let ok = json!({ "reply": { "reply": { "ok": true } } });
        let rules = |id: &str, pty: &str| {
            json!({
                "agents.signIns": { "reply": { "runs": [] } },
                "agents.signIn": { "reply": { "signInId": id },
                    "push": [{ "type": "agents.signIn", "signInId": id, "state": "running", "ptyId": pty }] },
                "daemon.open": { "reply": { "channel": "c1" } },
                "daemon.close": { "reply": {} },
                "agents.signInStop": { "reply": {} },
                "daemon.send:ping": ok,
                "daemon.send:pty.list": { "reply": { "reply": { "ok": true, "ptys": listed } } },
                "daemon.send:pty.attach": ok,
                "daemon.send:pty.resize": ok,
                "daemon.send:pty.write": ok,
            })
        };
        let door = Door::start(rules("si1", "p1"));
        cx.executor().allow_parking();
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
            cx.bind_keys([KeyBinding::new("escape", EscapePage, Some(&format!("{SHELL} && {SETTINGS_OPEN}")))]);
        });
        let runtime = runtime();
        let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), Some(door.client(&runtime)), runtime);
        let store = cx.update(|cx| cx.new(|_| Store::new(acts.clone())));
        cx.update(|cx| Store::of(&store, &acts, cx));
        let escapes = Rc::new(Cell::new(0));
        let shown = cx.update(|cx| cx.new(|_| Shown { store: store.clone(), escapes: escapes.clone() }));
        let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(shown, window, cx));
        waits(cx, "the store listening", |_| !door.asked_of("events.subscribe").is_empty());

        let start = Start::Run { agent: "opencode".into(), server: None, finish: None, pastes: false, cloud: false, terminal: true };
        store.update(cx, |s, cx| s.start(PLACE, &agent_row("opencode"), start, cx));
        waits(cx, "the login's shell attached over the computer's channel", |_| {
            daemon_frames(&door, "pty.attach").iter().any(|f| f["ptyId"] == "p1")
        });
        let asked = door.asked_of("agents.signIn");
        assert_eq!(asked[0]["terminal"], true, "asked as a sign-in in a terminal");
        assert_eq!(asked[0]["target"], json!({ "placeId": PLACE }));
        assert_eq!(door.asked_of("daemon.open")[0]["placeId"], PLACE, "the channel is that computer's");
        assert!(cx.debug_bounds("sign-in-terminal").is_some(), "the terminal stands in place under the row");

        waits(cx, "the terminal holding the focus", |cx| cx.update(|window, cx| window.focused(cx).is_some()));
        cx.simulate_keystrokes("escape");
        waits(cx, "Escape written to the login", |_| daemon_frames(&door, "pty.write").iter().any(|f| f["data"] == "\u{1b}"));
        assert_eq!(escapes.get(), 0, "Settings heard no Escape");

        door.rules(rules("si2", "p2"));
        let start = Start::Run { agent: "crush".into(), server: None, finish: None, pastes: false, cloud: false, terminal: true };
        store.update(cx, |s, cx| s.start(PLACE, &agent_row("crush"), start, cx));
        waits(cx, "a second sign-in's shell on the same channel", |_| {
            daemon_frames(&door, "pty.attach").iter().any(|f| f["ptyId"] == "p2")
        });
        store.update(cx, |s, cx| s.cancel(PLACE, &agent_row("opencode"), cx));
        cx.update(|window, _| window.refresh());
        for _ in 0..10 {
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(30));
        }
        assert!(door.asked_of("daemon.close").is_empty(), "the channel stands while another sign-in there holds it");
        assert!(cx.debug_bounds("sign-in-terminal").is_some(), "the other sign-in's terminal stays");
        store.update(cx, |s, cx| s.cancel(PLACE, &agent_row("crush"), cx));
        cx.update(|window, _| window.refresh());
        waits(cx, "the computer's channel closed as the terminal went", |_| {
            door.asked_of("daemon.close").iter().any(|f| f["channel"] == "c1")
        });
        assert!(cx.debug_bounds("sign-in-terminal").is_none(), "the terminal went with the sign-in");
    }
}
